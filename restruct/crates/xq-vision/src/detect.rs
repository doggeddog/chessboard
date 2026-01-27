use anyhow::{Context, Result};
use ndarray::{s, Array};
use ort::inputs;
use xcap::image::imageops::FilterType;
use xcap::image::{DynamicImage, GenericImageView};

use crate::input::VisionImage;
use crate::model::VisionModel;

pub const IMAGE_WIDTH: usize = 640;
pub const IMAGE_HEIGHT: usize = 640;

const CONFIDENCE_THRESHOLD: f32 = 0.7;
const IOU_THRESHOLD: f32 = 0.5;

/// 模型类别语义（与 Demo 保持一致）。
pub const LABELS: [char; 15] = [
    'n', 'b', 'a', 'k', 'r', 'c', 'p', 'R', 'N', 'A', 'K', 'B', 'C', 'P', '0',
];

/// 每类最多保留数量（与 Demo 保持一致）。
pub const LIMIT: [usize; 15] = [2, 2, 2, 1, 2, 2, 5, 2, 2, 2, 1, 2, 2, 5, 1];

/// 模型类型：large / rotate。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Large,
    Rotate,
}

impl Default for ModelKind {
    fn default() -> Self {
        #[cfg(feature = "rotate")]
        {
            Self::Rotate
        }
        #[cfg(not(feature = "rotate"))]
        {
            Self::Large
        }
    }
}

/// 单个检测框（模型坐标系，单位为模型输入像素）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detection {
    pub x0: f32,
    pub x1: f32,
    pub y0: f32,
    pub y1: f32,
    pub confidence: f32,
    pub label: char,

    pub(crate) class_idx: usize,
    pub(crate) area: f32,
}

impl Detection {
    fn new(x: f32, y: f32, w: f32, h: f32, class_idx: usize, confidence: f32) -> Self {
        let x0 = x - w / 2.0;
        let x1 = x + w / 2.0;
        let y0 = y - h / 2.0;
        let y1 = y + h / 2.0;
        Self {
            x0,
            x1,
            y0,
            y1,
            confidence,
            label: LABELS[class_idx],
            class_idx,
            area: w * h,
        }
    }

    #[must_use]
    pub fn center(self) -> (f32, f32) {
        ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0)
    }

    #[inline]
    fn iou(self, other: Self) -> f32 {
        let inter_width = (self.x1.min(other.x1) - self.x0.max(other.x0)).max(0.0);
        let inter_height = (self.y1.min(other.y1) - self.y0.max(other.y0)).max(0.0);
        let intersection = inter_width * inter_height;
        intersection / (self.area + other.area - intersection)
    }
}

/// 执行一次推理并返回 NMS 后的 detections。
pub fn predict(model: &VisionModel, origin_img: &VisionImage) -> Result<Vec<Detection>> {
    let img = DynamicImage::from(origin_img.clone()).resize_exact(
        IMAGE_WIDTH as u32,
        IMAGE_HEIGHT as u32,
        FilterType::Triangle,
    );

    let mut input = Array::zeros((1, 3, IMAGE_WIDTH, IMAGE_HEIGHT));
    for (x, y, pixel) in img.pixels() {
        let [r, g, b, _] = pixel.0;
        input[[0, 0, y as usize, x as usize]] = r as f32 / 255.0;
        input[[0, 1, y as usize, x as usize]] = g as f32 / 255.0;
        input[[0, 2, y as usize, x as usize]] = b as f32 / 255.0;
    }

    let outputs = model
        .session
        .run(inputs!["images" => input.view()]?)
        .context("ORT 推理执行失败")?;

    let output = outputs["output"]
        .try_extract_tensor::<f32>()
        .context("提取输出张量失败")?
        .view()
        .t()
        .slice(s![.., .., 0])
        .t()
        .to_owned();

    let mut detections: Vec<Detection> = output
        .rows()
        .into_iter()
        .filter_map(|row| {
            let (class_id, max_prob) = (5..20)
                .map(|idx| (idx - 5, row[idx]))
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap();

            let conf = row[4] * max_prob;
            if conf < CONFIDENCE_THRESHOLD {
                None
            } else {
                Some(Detection::new(
                    row[0],
                    row[1],
                    row[2],
                    row[3],
                    class_id,
                    conf,
                ))
            }
        })
        .collect();

    Ok(nms(&mut detections))
}

fn nms(detections: &mut Vec<Detection>) -> Vec<Detection> {
    detections.sort_unstable_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap());
    let mut filtered = Vec::with_capacity(33);
    let mut sizemap = [0usize; 15];

    while let Some(current) = detections.pop() {
        if sizemap[current.class_idx] + 1 > LIMIT[current.class_idx] {
            continue;
        }
        filtered.push(current);
        sizemap[current.class_idx] += 1;
        detections.retain(|d| current.iou(*d) < IOU_THRESHOLD);
    }

    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nms_respects_class_limits() {
        let mut dets = Vec::new();
        for i in 0..4 {
            dets.push(Detection::new(10.0 + i as f32, 10.0, 1.0, 1.0, 0, 0.99 - i as f32 * 0.01));
        }
        let filtered = nms(&mut dets);
        assert!(filtered.len() <= LIMIT[0]);
    }
}
