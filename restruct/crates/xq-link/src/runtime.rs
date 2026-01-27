use std::time::Duration;

use anyhow::{anyhow, Result};
use xq_core::Move;
use xq_vision::pipeline::{PipelineOutput, VisionPipeline};
use xq_vision::postprocess::BoardObservation;

use crate::geometry::BoardGeometry;
use crate::inject::{InputInjector, InputPlan};
use crate::sync::{ExternalUpdate, SyncState};
use crate::window::{LinkWindow, WindowPosition};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkRuntimeConfig {
    pub crop_padding_cells: f32,
    pub apply_scale_factor: bool,
    pub click_delay: Duration,
    pub confirm_after_inject: bool,
    pub require_legality: bool,
}

impl Default for LinkRuntimeConfig {
    fn default() -> Self {
        Self {
            crop_padding_cells: 1.0,
            apply_scale_factor: true,
            click_delay: Duration::from_millis(120),
            confirm_after_inject: true,
            require_legality: true,
        }
    }
}

pub struct LinkRuntime<I: InputInjector> {
    window: LinkWindow,
    pipeline: VisionPipeline,
    sync: SyncState,
    injector: I,
    config: LinkRuntimeConfig,
    last_geometry: Option<BoardGeometry>,
    last_observation: Option<BoardObservation>,
}

pub struct LinkStep {
    pub output: PipelineOutput,
    pub geometry: Option<BoardGeometry>,
    pub window: WindowPosition,
    pub update: Option<ExternalUpdate>,
}

pub struct InjectResult {
    pub plan: InputPlan,
    pub confirmed: Option<bool>,
    pub update: Option<ExternalUpdate>,
}

impl<I: InputInjector> LinkRuntime<I> {
    pub fn new(
        window: LinkWindow,
        pipeline: VisionPipeline,
        sync: SyncState,
        injector: I,
        config: LinkRuntimeConfig,
    ) -> Self {
        Self {
            window,
            pipeline,
            sync,
            injector,
            config,
            last_geometry: None,
            last_observation: None,
        }
    }

    pub fn last_observation(&self) -> Option<&BoardObservation> {
        self.last_observation.as_ref()
    }

    pub fn sync(&self) -> &SyncState {
        &self.sync
    }

    pub fn sync_mut(&mut self) -> &mut SyncState {
        &mut self.sync
    }

    pub fn capture_step(&mut self, confirm: bool) -> Result<LinkStep> {
        let frame = self.window.capture_frame()?;
        let output = if confirm {
            self.pipeline.analyze_with_confirm(&frame, &mut self.window)?
        } else {
            self.pipeline.analyze_frame(&frame)?
        };
        let window_pos = self.window.position()?;

        let geometry = output
            .crop_region
            .map(|region| BoardGeometry::from_crop(region, self.config.crop_padding_cells));

        if output.confirmed {
            if let Some(obs) = output.observation.clone() {
                self.last_observation = Some(obs);
            }
            if let Some(geom) = geometry {
                self.last_geometry = Some(geom);
            }
        }

        let update = if output.confirmed {
            output
                .observation
                .as_ref()
                .map(|obs| self.sync.ingest_external(obs.board.clone()))
        } else {
            None
        };

        Ok(LinkStep {
            output,
            geometry,
            window: window_pos,
            update,
        })
    }

    pub fn inject_move(&mut self, mv: Move) -> Result<InjectResult> {
        let geometry = self
            .last_geometry
            .ok_or_else(|| anyhow!("尚未建立棋盘几何信息"))?;
        let observation = self
            .last_observation
            .as_ref()
            .ok_or_else(|| anyhow!("尚未获得有效识别结果"))?;

        self.sync.set_verify_legality(self.config.require_legality);

        let pending = self.sync.prepare_injection(mv)?;
        let window_pos = self.window.position()?;

        let from = geometry.screen_point_for_pos(
            mv.from,
            observation.flipped,
            window_pos,
            self.config.apply_scale_factor,
        );
        let to = geometry.screen_point_for_pos(
            mv.to,
            observation.flipped,
            window_pos,
            self.config.apply_scale_factor,
        );
        let plan = InputPlan {
            from,
            to,
            delay: self.config.click_delay,
        };

        self.injector.click_move(&plan)?;

        if self.config.confirm_after_inject {
            let step = self.capture_step(true)?;
            let confirmed = step.output.confirmed
                && step
                    .output
                    .observation
                    .as_ref()
                    .map(|obs| obs.board == pending.expected)
                    .unwrap_or(false);
            return Ok(InjectResult {
                plan,
                confirmed: Some(confirmed),
                update: step.update,
            });
        }

        Ok(InjectResult {
            plan,
            confirmed: None,
            update: None,
        })
    }
}
