use std::fs;
use std::path::Path;

use once_cell::sync::OnceCell;
use serde::{de, Deserialize, Deserializer};
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, FromRepr};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, FromRepr)]
#[allow(missing_docs)]
/// Version of the batch to use.
pub enum BatchVersionID {
    V0 = 0x00,
}

/// A cutover entry for which batch version to use.
/// Version becomes active at start height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
struct Cutover {
    // Deserialize from a u8 in JSON -> BatchVersionID
    #[serde(deserialize_with = "u8_to_batch_version_id")]
    pub version: BatchVersionID,
    pub start: u64,
}

/// Convert a u8 to a `BatchVersionID`
fn u8_to_batch_version_id<'de, D>(d: D) -> Result<BatchVersionID, D::Error>
where
    D: Deserializer<'de>, {
    let raw = u8::deserialize(d)?;
    BatchVersionID::from_repr(raw).ok_or_else(|| {
        let supported: Vec<u8> = BatchVersionID::iter().map(|v| v as u8).collect();
        de::Error::custom(format!(
            "unsupported version id {raw}. Supported ids: {supported:?}"
        ))
    })
}

static BATCH_VERSION_CUTOVERS: OnceCell<Vec<Cutover>> = OnceCell::new();

/// Initialize batch version config to a single default cutover
pub fn default_config() -> eyre::Result<()> {
    let default = vec![Cutover {
        version: BatchVersionID::V0,
        start: 0,
    }];
    BATCH_VERSION_CUTOVERS.set(default).map_err(|_| {
        eyre::eyre!("Batch version config already initialized; initialize once at startup.")
    })
}

/// Initialize batch version config from a JSON file
pub fn from_file(path: &Path) -> eyre::Result<()> {
    let content = fs::read_to_string(path)
        .map_err(|e| eyre::eyre!("failed to read batch config '{}': {e}", path.display()))?;

    // Directly parse into Cutover (no intermediate struct)
    let mut cutovers: Vec<Cutover> = serde_json::from_str(&content).map_err(|e| {
        eyre::eyre!(
            r#"failed to parse '{}': {e}. Expected JSON array like
        [{{"version":0,"start":0}}, {{"version":1,"start":100}}]"#,
            path.display()
        )
    })?;

    if cutovers.is_empty() {
        eyre::bail!(
            "Batch config '{}' is empty. Provide at least one {{\"version\":V,\"start\":H}} entry.",
            path.display()
        );
    }

    // Ensure exactly one cutover per defined BatchVersionID
    let required_ids: Vec<u8> = BatchVersionID::iter().map(|v| v as u8).collect();

    cutovers.sort_by_key(|c| c.version as u8);

    let seen_ids: Vec<u8> = cutovers.iter().map(|c| c.version as u8).collect();
    if seen_ids != required_ids {
        eyre::bail!(
            "invalid batch config '{}': must include exactly one entry for each version {:?}, but saw {:?}.",
            path.display(), required_ids, seen_ids
        );
    }

    // Starts must strictly increase in version order
    for w in cutovers.windows(2) {
        let (vp, sp) = (w[0].version as u8, w[0].start);
        let (vn, sn) = (w[1].version as u8, w[1].start);
        if sn <= sp {
            eyre::bail!(
                "invalid batch config '{}': start heights must strictly increase (v{}: {} but v{}: {}).",
                path.display(), vp, sp, vn, sn
            );
        }
    }

    BATCH_VERSION_CUTOVERS.set(cutovers).map_err(|_| {
        eyre::eyre!("Batch version config already initialized; initialize once at startup.")
    })
}

/// Get the batch version for a given height
pub(crate) fn batch_version_for_height(height: u64) -> BatchVersionID {
    let cutovers = BATCH_VERSION_CUTOVERS
        .get()
        .expect("BATCH_VERSION_CUTOVERS not initialized; call init_batch_version_config_from_file() at startup");
    let idx = cutovers.partition_point(|c| c.start <= height);
    let active = if idx == 0 { 0 } else { idx - 1 };
    cutovers[active].version
}
