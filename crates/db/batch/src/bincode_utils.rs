use crate::ValueVersion;

/// Append 1-byte tag and then bincode-serialize.
pub(crate) fn serialize_versioned<T: serde::Serialize>(
    v: &T,
    ver: ValueVersion,
) -> eyre::Result<Vec<u8>> {
    let mut out = Vec::with_capacity(1 + bincode::serialized_size(v)? as usize);
    out.push(ver as u8); // 1-byte tag
    bincode::serialize_into(&mut out, v)?; // append payload
    Ok(out)
}

/// Strip 1-byte tag and return payload + version (tuple).
pub(crate) fn deserialize_versioned(bytes: &[u8]) -> eyre::Result<(ValueVersion, &[u8])> {
    let (tag, payload) = bytes
        .split_first()
        .ok_or_else(|| eyre::eyre!("empty value"))?;
    let ver = match *tag {
        0x00 => ValueVersion::V0,
        v => eyre::bail!("unknown version {v}"),
    };
    Ok((ver, payload))
}
