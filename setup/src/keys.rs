//! The key that signs CrownOS Android bundles.

/// minisign public key that `manifest.json.minisig` must verify against.
///
/// This is the placeholder development key, `crownos-android-dev` (key id
/// `013CAECD11A053F5`). `overlay/README.md` says how it was made and how the
/// release key replaces it.
pub const PUBLIC_KEY: &str = "RWT1U6ARza48AdBdnUdfFrGiqg4duI4bxpJ58L12bRZNYZLl7/KC9rJ1";

#[cfg(test)]
mod tests {
    use minisign_verify::{PublicKey, Signature};

    use super::*;

    const MESSAGE: &[u8] = b"crownos-android dev key";
    const SIGNATURE: &str = "untrusted comment: signature from minisign secret key
RUT1U6ARza48AWBakw0qN5MsC+/LCXYTdXoeJx8oXJhmERVEmFSVYsNPCA1Ld8cy+lA+DbkmxkROg4g+HIIIg9VjtO4XHjabrwI=
trusted comment: crownos-android dev key check
bsF47h+u8qPljcqY9YHBGSlIlfoAC73TO1pMtGtkY3on0DqtrwDvcDCr3n95n1/DWvzsnS0XZ/93tcaa1zyjAw==
";

    #[test]
    fn the_shipped_key_verifies_a_signature_made_with_its_secret()
    -> Result<(), minisign_verify::Error> {
        let key = PublicKey::from_base64(PUBLIC_KEY)?;
        key.verify(MESSAGE, &Signature::decode(SIGNATURE)?, false)
    }
}
