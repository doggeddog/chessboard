use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use xcap::image::{DynamicImage, ImageBuffer, Rgba};

/// 识别管线统一使用的图像类型（RGBA8）。
pub type VisionImage = ImageBuffer<Rgba<u8>, Vec<u8>>;

/// 帧来源描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameSource {
    Window { id: u32, title: String },
    ImageFile { path: PathBuf },
}

/// 一帧输入（图像与元信息）。
#[derive(Debug, Clone)]
pub struct Frame {
    pub image: VisionImage,
    pub width: u32,
    pub height: u32,
    pub source: FrameSource,
}

impl Frame {
    #[must_use]
    pub fn new(image: VisionImage, source: FrameSource) -> Self {
        let (width, height) = image.dimensions();
        Self {
            image,
            width,
            height,
            source,
        }
    }
}

/// 输入源抽象：窗口截图与图片文件复用同一管线。
pub trait CaptureInput {
    fn capture(&mut self) -> Result<Frame>;
}

/// 外部窗口截图输入源。
pub struct WindowCapture {
    window: xcap::Window,
    source: FrameSource,
}

impl WindowCapture {
    pub fn new(window: xcap::Window) -> Result<Self> {
        let id = window.id().context("无法读取窗口 id")?;
        let title = window.title().unwrap_or_else(|_| "<unknown>".to_string());
        let source = FrameSource::Window { id, title };
        Ok(Self { window, source })
    }
}

impl CaptureInput for WindowCapture {
    fn capture(&mut self) -> Result<Frame> {
        let image = self.window.capture_image().context("窗口截图失败")?;
        Ok(Frame::new(image, self.source.clone()))
    }
}

/// 图片文件输入源（用于上传图片识别开局等场景）。
pub struct ImageFile {
    path: PathBuf,
}

impl ImageFile {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn load_image(path: &Path) -> Result<DynamicImage> {
        xcap::image::open(path).with_context(|| format!("无法打开图片文件: {}", path.display()))
    }
}

impl CaptureInput for ImageFile {
    fn capture(&mut self) -> Result<Frame> {
        let img = Self::load_image(&self.path)?;
        let rgba = img.to_rgba8();
        let source = FrameSource::ImageFile {
            path: self.path.clone(),
        };
        Ok(Frame::new(rgba, source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_dimensions_match_image() {
        let img = VisionImage::from_pixel(4, 3, Rgba([0, 0, 0, 255]));
        let frame = Frame::new(
            img,
            FrameSource::ImageFile {
                path: PathBuf::from("dummy.png"),
            },
        );
        assert_eq!((frame.width, frame.height), (4, 3));
    }
}
