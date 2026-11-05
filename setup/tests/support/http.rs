use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};

#[derive(Default)]
struct State {
    files: HashMap<String, Vec<u8>>,
    cut_next_after: Option<usize>,
    ignore_ranges: bool,
    requests: Vec<(String, Option<u64>)>,
}

/// A one-request-per-connection HTTP/1.1 server with `Range: bytes=N-`
/// support and injectable faults.
pub struct Server {
    address: SocketAddr,
    state: Arc<Mutex<State>>,
}

impl Server {
    pub fn start(files: Vec<(&str, Vec<u8>)>) -> io::Result<Self> {
        Self::start_on(TcpListener::bind("127.0.0.1:0")?, files)
    }

    pub fn start_on(listener: TcpListener, files: Vec<(&str, Vec<u8>)>) -> io::Result<Self> {
        let address = listener.local_addr()?;
        let state = Arc::new(Mutex::new(State {
            files: files
                .into_iter()
                .map(|(path, bytes)| (path.to_owned(), bytes))
                .collect(),
            ..State::default()
        }));
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = serve(stream, &shared);
            }
        });
        Ok(Self { address, state })
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    /// The next response sends only `bytes` of its body, then hangs up.
    pub fn cut_next_response_after(&self, bytes: usize) {
        self.lock().cut_next_after = Some(bytes);
    }

    pub fn ignore_ranges(&self) {
        self.lock().ignore_ranges = true;
    }

    /// Every request so far: its path and the start of its Range, if any.
    pub fn requests(&self) -> Vec<(String, Option<u64>)> {
        self.lock().requests.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn serve(stream: TcpStream, state: &Mutex<State>) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let path = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("/")
        .to_owned();
    let mut range = None;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("range")
        {
            range = value
                .trim()
                .strip_prefix("bytes=")
                .and_then(|value| value.strip_suffix('-'))
                .and_then(|start| start.parse::<u64>().ok());
        }
    }
    let (body, status, extra, cut) = {
        let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
        state.requests.push((path.clone(), range));
        let ignore_ranges = state.ignore_ranges;
        let cut = state.cut_next_after.take();
        match state.files.get(&path) {
            None => (Vec::new(), "404 Not Found", String::new(), None),
            Some(file) => match range.filter(|_| !ignore_ranges) {
                None => (file.clone(), "200 OK", String::new(), cut),
                Some(start) if start >= file.len() as u64 => (
                    Vec::new(),
                    "416 Range Not Satisfiable",
                    format!("Content-Range: bytes */{}\r\n", file.len()),
                    None,
                ),
                Some(start) => (
                    file[start as usize..].to_vec(),
                    "206 Partial Content",
                    format!(
                        "Content-Range: bytes {start}-{}/{}\r\n",
                        file.len() - 1,
                        file.len()
                    ),
                    cut,
                ),
            },
        }
    };
    let mut stream = stream;
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n",
        body.len()
    )?;
    let sent = cut.map_or(body.len(), |cut| cut.min(body.len()));
    stream.write_all(&body[..sent])?;
    stream.flush()?;
    stream.shutdown(std::net::Shutdown::Both)
}
