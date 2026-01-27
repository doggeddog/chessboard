use std::thread;
use std::time::Duration;

use anyhow::Result;
use tracing::debug;
use xq_core::Side;

use crate::crop::{crop_image, CropLock, CropRegion};
use crate::detect::{predict, Detection, ModelKind, LABELS, IMAGE_HEIGHT, IMAGE_WIDTH};
use crate::input::{CaptureInput, Frame};
use crate::model::{ModelPaths, VisionModel};
use crate::postprocess::{detections_to_observation, BoardObservation, VisionError};
use crate::stability::{StabilityDecision, StabilityFilter, StabilitySettings};

/// 识别管线配置。
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub side_to_move: Side,
    pub model_kind: ModelKind,
    pub crop_padding_cells: f32,
    pub confirm_delay: Duration,
    pub stability: StabilitySettings,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            side_to_move: Side::Red,
            model_kind: ModelKind::default(),
            crop_padding_cells: 1.0,
            confirm_delay: Duration::from_millis(100),
            stability: StabilitySettings::default(),
        }
    }
}

/// 单帧识别输出（尚未进入上层状态机）。
#[derive(Debug, Clone)]
pub struct PipelineOutput {
    pub detections: Vec<Detection>,
    pub crop_region: Option<CropRegion>,
    pub observation: Option<BoardObservation>,
    pub stability: Option<StabilityDecision>,
    pub vision_error: Option<VisionError>,
    pub confirmed: bool,
}

/// 识别管线主结构：模型 + 裁剪锁定 + 稳定性过滤。
pub struct VisionPipeline {
    model: VisionModel,
    crop_lock: CropLock,
    stability: StabilityFilter,
    config: PipelineConfig,
}

impl VisionPipeline {
    pub fn new(paths: &ModelPaths, config: PipelineConfig) -> Result<Self> {
        let model = VisionModel::load(config.model_kind, paths)?;
        let crop_lock = CropLock::new(config.crop_padding_cells);
        let stability = StabilityFilter::new(config.stability);
        Ok(Self {
            model,
            crop_lock,
            stability,
            config,
        })
    }

    #[must_use]
    pub fn crop_region(&self) -> Option<CropRegion> {
        self.crop_lock.region()
    }

    pub fn reset_crop(&mut self) {
        self.crop_lock.clear();
    }

    pub fn reset_stability(&mut self) {
        self.stability.reset();
    }

    /// 单帧识别（不做二次确认）。
    pub fn analyze_frame(&mut self, frame: &Frame) -> Result<PipelineOutput> {
        let (detections, crop_region, observation, vision_error) =
            self.detect_and_observe(frame)?;

        let stability = observation
            .clone()
            .map(|obs| self.stability.process(obs));

        Ok(PipelineOutput {
            detections,
            crop_region,
            observation,
            stability,
            vision_error,
            confirmed: true,
        })
    }

    /// 单帧识别 + 二次确认（用于抖动过滤的前置步骤）。
    pub fn analyze_with_confirm(
        &mut self,
        frame: &Frame,
        confirmer: &mut dyn CaptureInput,
    ) -> Result<PipelineOutput> {
        let (detections, crop_region, observation, vision_error) =
            self.detect_and_observe(frame)?;

        let Some(primary_obs) = observation.clone() else {
            return Ok(PipelineOutput {
                detections,
                crop_region,
                observation,
                stability: None,
                vision_error,
                confirmed: false,
            });
        };

        thread::sleep(self.config.confirm_delay);
        let confirm_frame = confirmer.capture()?;
        let (_, _, confirm_obs, _) = self.detect_and_observe(&confirm_frame)?;

        let confirmed = confirm_obs
            .as_ref()
            .map(|obs| obs.board == primary_obs.board)
            .unwrap_or(false);

        if !confirmed {
            debug!(target: "xq_vision", "confirm step rejected this frame");
            return Ok(PipelineOutput {
                detections,
                crop_region,
                observation,
                stability: None,
                vision_error,
                confirmed: false,
            });
        }

        let stability = Some(self.stability.process(primary_obs));
        Ok(PipelineOutput {
            detections,
            crop_region,
            observation,
            stability,
            vision_error,
            confirmed: true,
        })
    }

    fn detect_and_observe(
        &mut self,
        frame: &Frame,
    ) -> Result<(Vec<Detection>, Option<CropRegion>, Option<BoardObservation>, Option<VisionError>)>
    {
        let full_detections = predict(&self.model, &frame.image)?;

        let crop_region = self
            .crop_lock
            .update_or_lock(frame.width, frame.height, &full_detections)
            .ok()
            .flatten();

        let (detections, region_locked) = if let Some(region) = crop_region {
            let cropped = crop_image(&frame.image, region);
            let mut dets = predict(&self.model, &cropped)?;
            dets = ensure_board_detection(dets, true);
            (dets, true)
        } else {
            (full_detections, false)
        };

        let detections = ensure_board_detection(detections, region_locked);

        match detections_to_observation(&detections, self.config.side_to_move) {
            Ok(obs) => Ok((detections, crop_region, Some(obs), None)),
            Err(err) => Ok((detections, crop_region, None, Some(err))),
        }
    }
}

fn ensure_board_detection(mut detections: Vec<Detection>, region_locked: bool) -> Vec<Detection> {
    let has_board = detections.iter().any(|d| d.label == '0');
    if has_board || !region_locked {
        return detections;
    }

    let class_idx = LABELS.iter().position(|&c| c == '0').unwrap();
    detections.push(Detection {
        x0: 0.0,
        y0: 0.0,
        x1: IMAGE_WIDTH as f32,
        y1: IMAGE_HEIGHT as f32,
        confidence: 1.0,
        label: '0',
        class_idx,
        area: (IMAGE_WIDTH as f32) * (IMAGE_HEIGHT as f32),
    });
    detections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_board_detection_inserts_when_locked() {
        let dets = Vec::<Detection>::new();
        let out = ensure_board_detection(dets, true);
        assert!(out.iter().any(|d| d.label == '0'));
    }

    #[test]
    fn ensure_board_detection_noop_when_not_locked() {
        let dets = Vec::<Detection>::new();
        let out = ensure_board_detection(dets, false);
        assert!(out.is_empty());
    }
}
