use anyhow::{anyhow, Result};
use xcap::image::imageops;

use crate::detect::{Detection, IMAGE_HEIGHT, IMAGE_WIDTH};
use crate::input::VisionImage;

/// 裁剪区域（基于原始截图坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CropRegion {
    #[must_use]
    pub fn clamp_to(self, max_width: u32, max_height: u32) -> Self {
        let x = self.x.min(max_width.saturating_sub(1));
        let y = self.y.min(max_height.saturating_sub(1));
        let max_w = max_width.saturating_sub(x);
        let max_h = max_height.saturating_sub(y);
        let width = self.width.min(max_w).max(1);
        let height = self.height.min(max_h).max(1);
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// 裁剪锁定：首次定位后复用同一裁剪区域。
#[derive(Debug, Clone)]
pub struct CropLock {
    region: Option<CropRegion>,
    padding_cells: f32,
}

impl CropLock {
    #[must_use]
    pub fn new(padding_cells: f32) -> Self {
        Self {
            region: None,
            padding_cells,
        }
    }

    #[must_use]
    pub fn region(&self) -> Option<CropRegion> {
        self.region
    }

    /// 若尚未锁定区域，则尝试用当前 detections 定位棋盘并锁定。
    pub fn update_or_lock(
        &mut self,
        origin_width: u32,
        origin_height: u32,
        detections: &[Detection],
    ) -> Result<Option<CropRegion>> {
        if let Some(region) = self.region {
            return Ok(Some(region));
        }

        let region = board_crop_region(origin_width, origin_height, detections, self.padding_cells)?;
        self.region = Some(region);
        Ok(self.region)
    }

    /// 手动清除锁定（例如窗口尺寸变化或连续异常）。
    pub fn clear(&mut self) {
        self.region = None;
    }
}

/// 根据棋盘框检测结果计算裁剪区域，并向外扩展半格。
pub fn board_crop_region(
    origin_width: u32,
    origin_height: u32,
    detections: &[Detection],
    padding_cells: f32,
) -> Result<CropRegion> {
    let board_det = detections
        .iter()
        .find(|d| d.label == '0')
        .copied()
        .ok_or_else(|| anyhow!("未识别到棋盘框(label=0)"))?;

    let scale_x = origin_width as f32 / IMAGE_WIDTH as f32;
    let scale_y = origin_height as f32 / IMAGE_HEIGHT as f32;

    let bx0 = (board_det.x0 * scale_x).max(0.0);
    let by0 = (board_det.y0 * scale_y).max(0.0);
    let bx1 = (board_det.x1 * scale_x).min(origin_width as f32);
    let by1 = (board_det.y1 * scale_y).min(origin_height as f32);

    let board_w = (bx1 - bx0).max(1.0);
    let board_h = (by1 - by0).max(1.0);

    let half_cell_x = board_w / 9.0 / 2.0 * padding_cells.max(0.0);
    let half_cell_y = board_h / 10.0 / 2.0 * padding_cells.max(0.0);

    let crop_x = (bx0 - half_cell_x).max(0.0) as u32;
    let crop_y = (by0 - half_cell_y).max(0.0) as u32;
    let x1p = (bx1 + half_cell_x).min(origin_width as f32);
    let y1p = (by1 + half_cell_y).min(origin_height as f32);

    let width = (x1p - crop_x as f32).max(1.0) as u32;
    let height = (y1p - crop_y as f32).max(1.0) as u32;

    Ok(CropRegion {
        x: crop_x,
        y: crop_y,
        width,
        height,
    }
    .clamp_to(origin_width, origin_height))
}

/// 从原图裁剪出子区域（用于锁定后提高推理效率）。
pub fn crop_image(image: &VisionImage, region: CropRegion) -> VisionImage {
    let (max_w, max_h) = image.dimensions();
    let region = region.clamp_to(max_w, max_h);
    imageops::crop_imm(image, region.x, region.y, region.width, region.height).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Detection;
    use xcap::image::{ImageBuffer, Rgba};

    fn board_detection() -> Detection {
        Detection {
            x0: 100.0,
            y0: 50.0,
            x1: 540.0,
            y1: 590.0,
            confidence: 0.99,
            label: '0',
            class_idx: 14,
            area: (540.0 - 100.0) * (590.0 - 50.0),
        }
    }

    #[test]
    fn board_crop_region_is_within_bounds() {
        let det = board_detection();
        let region = board_crop_region(1280, 720, &[det], 1.0).expect("应能计算裁剪区域");
        assert!(region.x < 1280 && region.y < 720);
        assert!(region.width > 0 && region.height > 0);
        assert!(region.x + region.width <= 1280);
        assert!(region.y + region.height <= 720);
    }

    #[test]
    fn crop_image_respects_region() {
        let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(10, 10, Rgba([0, 0, 0, 255]));
        let cropped = crop_image(
            &img,
            CropRegion {
                x: 2,
                y: 3,
                width: 4,
                height: 5,
            },
        );
        assert_eq!(cropped.dimensions(), (4, 5));
    }
}
