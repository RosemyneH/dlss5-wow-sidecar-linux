use std::fs;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFileEntry {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildManifest {
    pub version: String,
    pub platform: String,
    pub source_commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub neural_mvp_shader_sha256: Option<String>,
    pub files: Vec<ManifestFileEntry>,
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("read {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("write manifest: {0}")]
    Write(#[source] std::io::Error),
    #[error("serialize manifest: {0}")]
    Json(#[from] serde_json::Error),
}

fn sha256_file(path: &Path) -> Result<(u64, String), ManifestError> {
    let mut file = fs::File::open(path).map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let meta = file.metadata().map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(|source| ManifestError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok((meta.len(), format!("{:x}", hasher.finalize())))
}

pub fn build_manifest(
    bundle_dir: &Path,
    version: &str,
    source_commit: &str,
) -> Result<BuildManifest, ManifestError> {
    let mut files = Vec::new();
    let mut neural_mvp_shader_sha256 = None;

    if bundle_dir.is_dir() {
        let mut names: Vec<_> = fs::read_dir(bundle_dir)
            .map_err(|source| ManifestError::Io {
                path: bundle_dir.to_path_buf(),
                source,
            })?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();

        for name in names {
            let path = bundle_dir.join(&name);
            let (bytes, sha256) = sha256_file(&path)?;
            if name == "neural-mvp.frag.spv" {
                neural_mvp_shader_sha256 = Some(sha256.clone());
            }
            files.push(ManifestFileEntry {
                name,
                bytes,
                sha256,
            });
        }
    }

    Ok(BuildManifest {
        version: version.to_string(),
        platform: "linux".into(),
        source_commit: source_commit.to_string(),
        neural_mvp_shader_sha256,
        files,
    })
}

pub fn write_build_manifest(path: &Path, manifest: &BuildManifest) -> Result<(), ManifestError> {
    let json = serde_json::to_string_pretty(manifest).map_err(ManifestError::Json)?;
    fs::write(path, json).map_err(ManifestError::Write)?;
    Ok(())
}
