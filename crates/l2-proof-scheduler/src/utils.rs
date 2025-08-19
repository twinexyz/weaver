//! utils

/// convert vec to u64 bytes
pub fn to_bytes_u64(bytes: &[u8]) -> Result<[u8; 8], String> {
    if bytes.len() < size_of_val(&0u64) {
        return Err("size mismatch".to_string());
    }
    let array = [0u8; 8];

    for (i, mut _a) in array.iter().enumerate() {
        _a = &bytes[i]
    }

    Ok(array)
}
