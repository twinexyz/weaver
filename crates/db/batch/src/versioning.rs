use std::fs;
use std::path::Path;

use once_cell::sync::OnceCell;
use serde::Deserialize;

#[repr(u8)]
#[allow(missing_docs)]
#[derive(Debug, Clone)]
/// Version to serialize with
pub enum ValueVersion {
    V0 = 0x00,
}

/// Logical versions keyed by start height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BatchVersion {
    V0 { start: u64 },
}

impl BatchVersion {
    #[inline]
    fn get_start_height(self) -> u64 {
        match self {
            BatchVersion::V0 { start } => start,
        }
    }
}

#[allow(missing_docs)]
#[derive(Debug, Deserialize)]
pub(self) struct BatchVersionConfig {
    pub version: u8,
    pub start: u64,
}

impl TryFrom<BatchVersionConfig> for BatchVersion {
    type Error = eyre::Report;

    fn try_from(c: BatchVersionConfig) -> Result<Self, Self::Error> {
        Ok(match c.version {
            0 => BatchVersion::V0 { start: c.start },
            other => eyre::bail!("unsupported version id {other} (max supported is 3)"),
        })
    }
}

/// Global batch version config initialized once at startup.
static BATCH_VERSION_CONFIG: OnceCell<Vec<BatchVersion>> = OnceCell::new();

/// Initialize batch version config from a file
pub fn init_batch_version_config_from_file(path: &Path) -> eyre::Result<()> {
    let content = fs::read_to_string(path).map_err(|e| {
        eyre::eyre!(
            "failed to read forks file at '{}': {e}. \
             Make sure the file exists and is readable.",
            path.display()
        )
    })?;

    let mut items: Vec<BatchVersionConfig> = serde_json::from_str(&content).map_err(|e| {
        eyre::eyre!(
            "failed to parse forks file at '{}': {e}. \
             Expected JSON array like: \
             [{{\"version\":0,\"start\":0}}, {{\"version\":1,\"start\":100}}]",
            path.display()
        )
    })?;

    if items.is_empty() {
        eyre::bail!(
            "forks file at '{}' is empty. Provide at least one {{\"version\":V, \"start\":H}} entry.",
            path.display()
        );
    }

    // sort by version id for deterministic order (0,1,2,...)
    items.sort_by_key(|c| c.version);

    // validate monotonic version ids and strictly increasing starts
    for w in items.windows(2) {
        let (vp, sp) = (w[0].version, w[0].start);
        let (vn, sn) = (w[1].version, w[1].start);
        if vn != vp + 1 {
            eyre::bail!(
                "invalid batch version config in '{}': version ids must be consecutive (found {vp} then {vn}). \
                 Fix the JSON so versions go 0,1,2,... with no gaps.",
                path.display()
            );
        }
        if sn <= sp {
            eyre::bail!(
                "invalid batch version config in '{}': start heights must strictly increase (v{}@{} then v{}@{}). \
                 Ensure each later version has a larger 'start' height.",
                path.display(),
                vp, sp, vn, sn
            );
        }
    }

    let mut config = Vec::with_capacity(items.len());
    for c in items {
        config.push(BatchVersion::try_from(c)?);
    }

    BATCH_VERSION_CONFIG.set(config).map_err(|_| {
        eyre::eyre!(
            "Batch version config was already initialized. \
             Initialize it once at startup before using batch DB."
        )
    })
}

pub(crate) fn value_version_for_height(height: u64) -> ValueVersion {
    let batch_version_config = BATCH_VERSION_CONFIG.get().expect(
        "BATCH_VERSION_CONFIG not initialized, call init_batch_version_config_from_file() at startup",
    );

    // Binary search over strictly-increasing starts
    let idx = batch_version_config.partition_point(|version| version.get_start_height() <= height);
    let active = if idx == 0 { 0 } else { idx - 1 };
    match batch_version_config[active] {
        BatchVersion::V0 { .. } => ValueVersion::V0,
    }
}
