use std::process::{Command, Stdio};

/// Opens `url` in the user's browser without waiting for it.
pub fn open_url(url: &str) {
    let spawned = Command::new("xdg-open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    match spawned {
        Ok(mut child) => {
            let reaper = std::thread::Builder::new()
                .name("xdg-open".into())
                .spawn(move || child.wait());
            if let Err(error) = reaper {
                log::warn!("could not wait for xdg-open: {error}");
            }
        }
        Err(error) => log::warn!("could not open {url}: {error}"),
    }
}
