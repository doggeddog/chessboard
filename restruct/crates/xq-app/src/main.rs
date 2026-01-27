//! xq-app: 正式版应用入口骨架（第 1 步）。

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{info, warn};

fn main() -> Result<()> {
    init_tracing();

    info!(target: "xq_app", "starting xq-app skeleton");

    let resources = ResourcePaths::detect()?;
    let report = resources.self_check();
    report.log();

    // 最小连通性检查：确保 workspace crate 都能被调用。
    info!(target: "xq_app", "core: {}", xq_core::core_version());
    info!(target: "xq_app", "vision: {}", xq_vision::vision_healthcheck());
    info!(target: "xq_app", "engine: {}", xq_engine::engine_healthcheck());
    info!(target: "xq_app", "link: {}", xq_link::link_healthcheck());

    if report.has_missing() {
        warn!(target: "xq_app", "resource self-check failed; see logs above");
    } else {
        info!(target: "xq_app", "resource self-check passed");
    }

    Ok(())
}

fn init_tracing() {
    // 忽略重复初始化错误，便于未来在测试/嵌入场景复用。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}

#[derive(Debug, Clone)]
struct ResourcePaths {
    repo_root: PathBuf,
    large_model: PathBuf,
    rotate_model: PathBuf,
    pikafish_bin: PathBuf,
    pikafish_nnue: PathBuf,
}

impl ResourcePaths {
    fn detect() -> Result<Self> {
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
            large_model: libs_dir.join("large.onnx"),
            rotate_model: libs_dir.join("rotate.onnx"),
            pikafish_bin: pikafish_dir.join(platform_pikafish_bin_name()),
            pikafish_nnue: pikafish_dir.join("pikafish.nnue"),
        })
    }

    fn self_check(&self) -> ResourceCheckReport {
        let mut missing = Vec::new();

        check_file(&self.large_model, "large.onnx", &mut missing);
        check_file(&self.rotate_model, "rotate.onnx", &mut missing);
        check_file(&self.pikafish_nnue, "pikafish.nnue", &mut missing);
        check_file(&self.pikafish_bin, "pikafish binary", &mut missing);

        ResourceCheckReport {
            repo_root: self.repo_root.clone(),
            missing,
        }
    }
}

#[derive(Debug, Clone)]
struct ResourceCheckReport {
    repo_root: PathBuf,
    missing: Vec<String>,
}

impl ResourceCheckReport {
    fn has_missing(&self) -> bool {
        !self.missing.is_empty()
    }

    fn log(&self) {
        info!(target: "xq_app", "repository root detected at: {}", self.repo_root.display());
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

fn platform_pikafish_bin_name() -> &'static str {
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
