use std::path::Path;

use anyhow::{Context, Result};
use tempfile::NamedTempFile;

pub(crate) fn temporary(path: &Path, kind: &str) -> Result<NamedTempFile> {
    let directory = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    NamedTempFile::new_in(directory)
        .with_context(|| format!("create a temporary {kind} for {}", path.display()))
}
