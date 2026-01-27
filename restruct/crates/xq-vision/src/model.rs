use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, Result};
use ort::session::builder::GraphOptimizationLevel;
use tracing::info;

use crate::detect::ModelKind;

static ORT_INIT: OnceLock<()> = OnceLock::new();

#[cfg(target_os = "macos")]
use ort::execution_providers::CoreMLExecutionProvider;
#[cfg(all(target_os = "windows", feature = "gpu"))]
use ort::execution_providers::{CUDAExecutionProvider, DirectMLExecutionProvider};
#[cfg(target_os = "linux")]
use ort::execution_providers::CUDAExecutionProvider;

/// 模型路径集合：正式版从 `libs/` 读取模型文件。
#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub large: PathBuf,
    pub rotate: PathBuf,
}

impl ModelPaths {
    #[must_use]
    pub fn from_libs_dir(libs_dir: &Path) -> Self {
        Self {
            large: libs_dir.join("large.onnx"),
            rotate: libs_dir.join("rotate.onnx"),
        }
    }

    #[must_use]
    pub fn path_for(&self, kind: ModelKind) -> &Path {
        match kind {
            ModelKind::Large => &self.large,
            ModelKind::Rotate => &self.rotate,
        }
    }
}

/// 已加载的视觉模型（ORT session 封装）。
pub struct VisionModel {
    pub(crate) session: ort::session::Session,
    pub kind: ModelKind,
}

impl VisionModel {
    pub fn load(kind: ModelKind, paths: &ModelPaths) -> Result<Self> {
        ensure_ort_initialized()?;

        let model_path = paths.path_for(kind);
        let bytes = fs::read(model_path)
            .with_context(|| format!("读取模型文件失败: {}", model_path.display()))?;

        let session = ort::session::Session::builder()
            .context("创建 ORT SessionBuilder 失败")?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .context("设置 ORT 优化等级失败")?
            .commit_from_memory(&bytes)
            .context("从内存加载 ONNX 模型失败")?;

        info!(target: "xq_vision", ?kind, path = %model_path.display(), "vision model loaded");
        Ok(Self { session, kind })
    }
}

fn ensure_ort_initialized() -> Result<()> {
    ORT_INIT.get_or_init(|| {
        #[cfg(all(target_os = "windows", feature = "gpu"))]
        let eps = [
            CUDAExecutionProvider::default().build(),
            DirectMLExecutionProvider::default().build(),
        ];

        #[cfg(all(target_os = "windows", not(feature = "gpu")))]
        let eps = [];

        #[cfg(target_os = "macos")]
        let eps = [CoreMLExecutionProvider::default().build()];

        #[cfg(target_os = "linux")]
        let eps = [CUDAExecutionProvider::default().build()];

        ort::init()
            .with_execution_providers(eps)
            .commit()
            .expect("初始化 ORT 环境失败");
    });
    Ok(())
}
