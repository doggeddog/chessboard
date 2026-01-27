//! xq-vision：截图输入、YOLO 推理与识别稳定性管线（Step 4）。

pub mod crop;
pub mod detect;
pub mod input;
pub mod model;
pub mod pipeline;
pub mod postprocess;
pub mod stability;

pub use crop::{board_crop_region, crop_image, CropLock, CropRegion};
pub use detect::{predict, Detection, ModelKind, IMAGE_HEIGHT, IMAGE_WIDTH};
pub use input::{CaptureInput, Frame, FrameSource, ImageFile, VisionImage, WindowCapture};
pub use model::{ModelPaths, VisionModel};
pub use pipeline::{PipelineConfig, PipelineOutput, VisionPipeline};
pub use postprocess::{detections_to_grid, detections_to_observation, BoardObservation, Camp, VisionError};
pub use stability::{RejectReason, StabilityDecision, StabilityFilter, StabilitySettings};

/// 识别模块的最小健康检查。
///
/// 这里避免触发真实推理，仅验证依赖连通与核心常量可用。
pub fn vision_healthcheck() -> &'static str {
    let _ = xq_core::core_version();
    let _ = (IMAGE_WIDTH, IMAGE_HEIGHT);
    "xq-vision/ok"
}

#[cfg(test)]
mod tests {
    use super::vision_healthcheck;

    #[test]
    fn vision_healthcheck_returns_ok() {
        assert_eq!(vision_healthcheck(), "xq-vision/ok");
    }
}
