use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

const CHUNK: usize = 256 * 1024;

/// Copies `reader` into `writer` and returns the lower-case hex sha256 of the
/// bytes copied, reading every byte once.
pub fn copy(reader: &mut impl Read, writer: &mut impl Write) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut chunk = vec![0; CHUNK];
    loop {
        let read = match reader.read(&mut chunk) {
            Ok(0) => return Ok(hex::encode(hasher.finalize())),
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        hasher.update(&chunk[..read]);
        writer.write_all(&chunk[..read])?;
    }
}

/// Feeds every byte of `reader` into `hasher`.
pub fn update_from(hasher: &mut Sha256, reader: &mut impl Read) -> io::Result<u64> {
    io::copy(reader, &mut HashWriter(hasher))
}

/// Lower-case hex sha256 of the file at `path`.
pub fn of_file(path: &Path) -> io::Result<String> {
    let mut hasher = Sha256::new();
    update_from(&mut hasher, &mut File::open(path)?)?;
    Ok(hex::encode(hasher.finalize()))
}

struct HashWriter<'a>(&'a mut Sha256);

impl Write for HashWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn copying_hashes_what_it_copies() -> io::Result<()> {
        let mut copied = Vec::new();
        assert_eq!(copy(&mut &b"abc"[..], &mut copied)?, ABC);
        assert_eq!(copied, b"abc");
        Ok(())
    }

    #[test]
    fn hashing_a_reader_matches_the_known_digest() -> io::Result<()> {
        let mut hasher = Sha256::new();
        assert_eq!(update_from(&mut hasher, &mut &b"abc"[..])?, 3);
        assert_eq!(hex::encode(hasher.finalize()), ABC);
        Ok(())
    }
}
