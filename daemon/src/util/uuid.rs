use std::io;

use rustix::rand::{GetRandomFlags, getrandom};

const VERSION_4: u8 = 0x40;
const VARIANT_RFC4122: u8 = 0x80;

/// A random version 4 UUID in its hyphenated form.
pub fn random_v4() -> io::Result<String> {
    let mut bytes = [0u8; 16];
    let mut filled = 0;
    while filled < bytes.len() {
        filled += getrandom(&mut bytes[filled..], GetRandomFlags::empty())?;
    }
    Ok(format_v4(bytes))
}

fn format_v4(mut bytes: [u8; 16]) -> String {
    bytes[6] = (bytes[6] & 0x0f) | VERSION_4;
    bytes[8] = (bytes[8] & 0x3f) | VARIANT_RFC4122;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_version_and_variant() {
        assert_eq!(
            format_v4([0xff; 16]),
            "ffffffff-ffff-4fff-bfff-ffffffffffff"
        );
        assert_eq!(format_v4([0; 16]), "00000000-0000-4000-8000-000000000000");
    }

    #[test]
    fn two_ids_differ() -> io::Result<()> {
        assert_ne!(random_v4()?, random_v4()?);
        Ok(())
    }
}
