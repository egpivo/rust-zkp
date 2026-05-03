use sha2::{Digest, Sha256};

pub fn fs_piece(hasher: &mut Sha256, tag: u8, bytes: &[u8]) {
    hasher.update([tag]);
    hasher.update((bytes.len() as u32).to_be_bytes());
    hasher.update(bytes);
}
