use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use blake2::{Blake2b512, Digest};
use ring::signature::{Ed25519KeyPair, KeyPair};

/// A minisign key pair for tests, signing exactly as `minisign -S` does
/// (prehashed `ED` signatures).
pub struct TestKey {
    pair: Ed25519KeyPair,
    key_id: [u8; 8],
}

impl TestKey {
    pub fn new(seed: u8) -> Self {
        let pair =
            Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap_or_else(|_| unreachable!());
        Self {
            pair,
            key_id: [seed; 8],
        }
    }

    pub fn public_key(&self) -> String {
        let mut bytes = b"Ed".to_vec();
        bytes.extend_from_slice(&self.key_id);
        bytes.extend_from_slice(self.pair.public_key().as_ref());
        STANDARD.encode(bytes)
    }

    pub fn sign(&self, message: &[u8]) -> String {
        let signature = self.pair.sign(&Blake2b512::digest(message));
        let trusted = "timestamp:0\tfile:manifest.json";
        let mut global = signature.as_ref().to_vec();
        global.extend_from_slice(trusted.as_bytes());
        let global = self.pair.sign(&global);
        let mut line = b"ED".to_vec();
        line.extend_from_slice(&self.key_id);
        line.extend_from_slice(signature.as_ref());
        format!(
            "untrusted comment: test signature\n{}\ntrusted comment: {trusted}\n{}\n",
            STANDARD.encode(line),
            STANDARD.encode(global.as_ref())
        )
    }
}
