//! Directory watches on one inotify descriptor, for the daemon's poll loop.

use std::collections::HashMap;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::Path;

use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};

const EVENT_BUFFER: usize = 4096;

/// A set of watched directories, each tagged with what it stands for.
pub struct DirWatch<T> {
    fd: OwnedFd,
    tags: HashMap<i32, T>,
}

impl<T: Copy + Eq> DirWatch<T> {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            fd: inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)?,
            tags: HashMap::new(),
        })
    }

    /// Watches `dir` for files being written, moved in or out, or deleted.
    pub fn add(&mut self, dir: &Path, tag: T) -> io::Result<()> {
        let flags = WatchFlags::CLOSE_WRITE
            | WatchFlags::MOVED_TO
            | WatchFlags::MOVED_FROM
            | WatchFlags::CREATE
            | WatchFlags::DELETE
            | WatchFlags::ONLYDIR;
        let wd = inotify::add_watch(&self.fd, dir, flags)?;
        self.tags.insert(wd, tag);
        Ok(())
    }

    pub fn is_watching(&self, tag: T) -> bool {
        self.tags.values().any(|watched| *watched == tag)
    }

    /// The tags of every directory that changed since the last call, once each.
    pub fn drain(&mut self) -> io::Result<Vec<T>> {
        let mut buffer = [MaybeUninit::uninit(); EVENT_BUFFER];
        let mut reader = inotify::Reader::new(&self.fd, &mut buffer);
        let mut changed = Vec::new();
        let mut gone = Vec::new();
        loop {
            let event = match reader.next() {
                Ok(event) => event,
                Err(rustix::io::Errno::AGAIN) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            if event.events().contains(ReadFlags::IGNORED) {
                gone.push(event.wd());
            }
            if let Some(tag) = self.tags.get(&event.wd()).copied()
                && !changed.contains(&tag)
            {
                changed.push(tag);
            }
        }
        for wd in gone {
            self.tags.remove(&wd);
        }
        Ok(changed)
    }
}

impl<T> AsFd for DirWatch<T> {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Topic {
        Setup,
        Apps,
    }

    #[test]
    fn reports_each_changed_directory_once() -> io::Result<()> {
        let root = std::env::temp_dir().join(format!("runtime-watch-{}", std::process::id()));
        let (setup, apps) = (root.join("setup"), root.join("apps"));
        std::fs::create_dir_all(&setup)?;
        std::fs::create_dir_all(&apps)?;
        let mut watch = DirWatch::new()?;
        watch.add(&setup, Topic::Setup)?;
        watch.add(&apps, Topic::Apps)?;
        assert_eq!(watch.drain()?, []);
        std::fs::write(setup.join("setup.json"), "{}")?;
        std::fs::write(setup.join("pending"), "")?;
        assert_eq!(watch.drain()?, [Topic::Setup]);
        std::fs::remove_dir_all(&apps)?;
        assert_eq!(watch.drain()?, [Topic::Apps]);
        assert!(!watch.is_watching(Topic::Apps));
        std::fs::remove_dir_all(&root)?;
        Ok(())
    }
}
