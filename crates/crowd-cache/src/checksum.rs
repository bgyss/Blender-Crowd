//! Algorithms used for cache integrity and source identity.

pub fn payload_checksum(bytes: &[u8]) -> u32 {
    crc32c::crc32c(bytes)
}

pub fn content_hash(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}
