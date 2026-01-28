use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ResourcePaths {
    repo_root: PathBuf,
    libs_dir: PathBuf,
    large_model: PathBuf,
    rotate_model: PathBuf,
    pikafish_bin: PathBuf,
    pikafish_nnue: PathBuf,
}

impl ResourcePaths {
    pub fn detect() -> Result<Self> {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .context("unable to resolve workspace root from CARGO_MANIFEST_DIR")?;
        let repo_root = workspace_root
            .parent()
            .context("unable to resolve repository root from workspace root")?
            .to_path_buf();

        let libs_dir = repo_root.join("libs");
        let pikafish_dir = libs_dir.join("pikafish");

        Ok(Self {
            repo_root,
            libs_dir: libs_dir.clone(),
            large_model: libs_dir.join("large.onnx"),
            rotate_model: libs_dir.join("rotate.onnx"),
            pikafish_bin: pikafish_dir.join(platform_pikafish_bin_name()),
            pikafish_nnue: pikafish_dir.join("pikafish.nnue"),
        })
    }

    pub fn libs_dir(&self) -> &Path {
        &self.libs_dir
    }

    pub fn pikafish_bin(&self) -> &Path {
        &self.pikafish_bin
    }

    pub fn pikafish_nnue(&self) -> &Path {
        &self.pikafish_nnue
    }

    pub fn self_check(&self) -> ResourceCheckReport {
        let mut missing = Vec::new();

        check_file(&self.large_model, "large.onnx", &mut missing);
        check_file(&self.rotate_model, "rotate.onnx", &mut missing);
        check_file(&self.pikafish_nnue, "pikafish.nnue", &mut missing);
        check_file(&self.pikafish_bin, "pikafish binary", &mut missing);

        ResourceCheckReport {
            repo_root: Some(self.repo_root.clone()),
            missing,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResourceCheckReport {
    pub repo_root: Option<PathBuf>,
    pub missing: Vec<String>,
    pub error: Option<String>,
}

impl ResourceCheckReport {
    pub fn from_error(err: anyhow::Error) -> Self {
        Self {
            repo_root: None,
            missing: Vec::new(),
            error: Some(err.to_string()),
        }
    }

    pub fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }

    pub fn log(&self) {
        if let Some(root) = &self.repo_root {
            info!(target: "xq_app", "repository root detected at: {}", root.display());
        }
        if let Some(err) = &self.error {
            warn!(target: "xq_app", "resource detection failed: {err}");
            return;
        }
        if self.missing.is_empty() {
            info!(target: "xq_app", "all required resources are present");
            return;
        }

        for item in &self.missing {
            warn!(target: "xq_app", "missing resource: {item}");
        }
    }
}

fn check_file(path: &Path, label: &str, missing: &mut Vec<String>) {
    if path.is_file() {
        info!(target: "xq_app", "resource ok: {} -> {}", label, path.display());
        return;
    }

    missing.push(format!("{label} ({})", path.display()));
}

pub fn platform_pikafish_bin_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "pikafish-macos"
    }

    #[cfg(target_os = "linux")]
    {
        "pikafish-linux"
    }

    #[cfg(target_os = "windows")]
    {
        "pikafish-windows.exe"
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "pikafish-unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::platform_pikafish_bin_name;

    #[test]
    fn platform_bin_name_is_defined() {
        assert!(!platform_pikafish_bin_name().is_empty());
    }
}
