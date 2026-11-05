use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::Context;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HTTPS_PORT: u16 = 443;
const HTTP_PORT: u16 = 80;

/// Pauses that double from `first` up to `max`.
#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    pub first: Duration,
    pub max: Duration,
}

impl Backoff {
    /// The schedule setup uses on a real network.
    pub const NETWORK: Self = Self {
        first: Duration::from_secs(5),
        max: Duration::from_secs(300),
    };

    pub fn pauses(self) -> Pauses {
        Pauses {
            next: self.first,
            max: self.max,
        }
    }
}

/// A running [`Backoff`].
pub struct Pauses {
    next: Duration,
    max: Duration,
}

impl Pauses {
    pub fn sleep(&mut self) {
        std::thread::sleep(self.next);
        self.next = (self.next * 2).min(self.max);
    }
}

/// `host:port` of an `http` or `https` URL.
pub fn authority(url: &str) -> anyhow::Result<String> {
    let uri: ureq::http::Uri = url.parse().with_context(|| format!("{url} is not a URL"))?;
    let host = uri.host().with_context(|| format!("{url} names no host"))?;
    let default_port = if uri.scheme_str() == Some("https") {
        HTTPS_PORT
    } else {
        HTTP_PORT
    };
    Ok(format!("{host}:{}", uri.port_u16().unwrap_or(default_port)))
}

/// Whether `authority` resolves and accepts a TCP connection.
pub fn reachable(authority: &str) -> bool {
    authority.to_socket_addrs().is_ok_and(|mut addresses| {
        addresses.any(|address| TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).is_ok())
    })
}

/// Blocks until `authority` is reachable, calling `waiting` before each pause.
pub fn wait_until_reachable(
    authority: &str,
    backoff: Backoff,
    mut waiting: impl FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut pauses = backoff.pauses();
    while !reachable(authority) {
        waiting()?;
        pauses.sleep();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorities_carry_the_scheme_default_port() -> anyhow::Result<()> {
        assert_eq!(
            authority("https://sourceforge.net/projects/waydroid/files/x.zip/download")?,
            "sourceforge.net:443"
        );
        assert_eq!(authority("http://127.0.0.1:8080/x.zip")?, "127.0.0.1:8080");
        assert!(authority("not a url").is_err());
        Ok(())
    }

    #[test]
    fn waiting_ends_once_the_server_listens() -> anyhow::Result<()> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?.to_string();
        drop(listener);
        let fast = Backoff {
            first: Duration::from_millis(5),
            max: Duration::from_millis(20),
        };
        let mut waits = 0;
        let mut server = None;
        wait_until_reachable(&address, fast, || {
            waits += 1;
            if waits == 3 {
                server = Some(std::net::TcpListener::bind(&address)?);
            }
            Ok(())
        })?;
        assert_eq!(waits, 3);
        assert!(server.is_some());
        Ok(())
    }
}
