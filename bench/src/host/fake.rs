use std::io;
use std::path::{Path, PathBuf};

use super::Host;

/// A fake `/proc` and `/sys` in a fresh directory, removed again on drop.
pub struct FakeHost {
    root: PathBuf,
}

impl FakeHost {
    pub fn new(name: &str) -> io::Result<Self> {
        let root =
            std::env::temp_dir().join(format!("crown-android-bench-{name}-{}", std::process::id()));
        if root.exists() {
            std::fs::remove_dir_all(&root)?;
        }
        std::fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn host(&self) -> Host {
        Host::rooted(&self.root)
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn file(&self, relative: &str, contents: &str) -> io::Result<()> {
        write(&self.path(relative), contents)
    }

    /// A process with a command line, a parent and a cgroup, listed in that cgroup's `cgroup.procs`.
    pub fn process(&self, pid: u32, argv: &[&str], parent: u32, cgroup: &str) -> io::Result<()> {
        let directory = self.path(&format!("proc/{pid}"));
        let command_line: String = argv.iter().flat_map(|argument| [*argument, "\0"]).collect();
        write(&directory.join("cmdline"), &command_line)?;
        let name = argv
            .first()
            .and_then(|program| program.rsplit('/').next())
            .unwrap_or("");
        write(&directory.join("comm"), &format!("{name}\n"))?;
        write(
            &directory.join("status"),
            &format!("Name:\t{name}\nPPid:\t{parent}\n"),
        )?;
        write(&directory.join("cgroup"), &format!("0::{cgroup}\n"))?;
        let procs = self
            .path("sys/fs/cgroup")
            .join(cgroup.trim_start_matches('/'))
            .join("cgroup.procs");
        let listed = std::fs::read_to_string(&procs).unwrap_or_default();
        write(&procs, &format!("{listed}{pid}\n"))
    }
}

impl Drop for FakeHost {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn write(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
