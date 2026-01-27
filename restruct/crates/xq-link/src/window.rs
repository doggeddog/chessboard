use anyhow::{anyhow, Context, Result};
use tracing::debug;
use xcap::Window as XcapWindow;
use xq_vision::input::{CaptureInput, Frame, FrameSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkWindowInfo {
    pub id: u32,
    pub title: String,
    pub app_name: String,
    pub width: u32,
    pub height: u32,
}

impl LinkWindowInfo {
    fn from_window(win: &XcapWindow) -> Result<Self> {
        Ok(Self {
            id: win.id().context("无法读取窗口 id")?,
            title: win.title().unwrap_or_else(|_| "<unknown>".to_string()),
            app_name: win.app_name().unwrap_or_else(|_| "<unknown>".to_string()),
            width: win.width().context("无法读取窗口宽度")?,
            height: win.height().context("无法读取窗口高度")?,
        })
    }
}

pub fn list_windows() -> Result<Vec<LinkWindowInfo>> {
    let windows = XcapWindow::all().context("无法枚举窗口列表")?;
    let mut result = Vec::with_capacity(windows.len());
    for window in windows {
        match LinkWindowInfo::from_window(&window) {
            Ok(info) => result.push(info),
            Err(err) => debug!(target: "xq_link", error = %err, "skip invalid window"),
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

pub struct LinkWindow {
    window: XcapWindow,
    info: LinkWindowInfo,
    source: FrameSource,
}

impl LinkWindow {
    pub fn from_info(info: &LinkWindowInfo) -> Result<Self> {
        let windows = XcapWindow::all().context("无法枚举窗口列表")?;
        let window = windows
            .into_iter()
            .find(|w| w.id().ok() == Some(info.id))
            .ok_or_else(|| anyhow!("未找到目标窗口: {}", info.id))?;

        let source = FrameSource::Window {
            id: info.id,
            title: info.title.clone(),
        };

        Ok(Self {
            window,
            info: info.clone(),
            source,
        })
    }

    pub fn info(&self) -> &LinkWindowInfo {
        &self.info
    }

    pub fn position(&self) -> Result<WindowPosition> {
        let x = self.window.x().context("无法读取窗口坐标 x")?;
        let y = self.window.y().context("无法读取窗口坐标 y")?;
        let width = self.window.width().context("无法读取窗口宽度")?;
        let height = self.window.height().context("无法读取窗口高度")?;
        let scale_factor = self
            .window
            .current_monitor()
            .and_then(|m| m.scale_factor())
            .unwrap_or(1.0);

        Ok(WindowPosition {
            x,
            y,
            width,
            height,
            scale_factor,
        })
    }

    pub fn capture_frame(&mut self) -> Result<Frame> {
        let image = self.window.capture_image().context("窗口截图失败")?;
        Ok(Frame::new(image, self.source.clone()))
    }
}

impl CaptureInput for LinkWindow {
    fn capture(&mut self) -> Result<Frame> {
        self.capture_frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_position_is_copy() {
        let pos = WindowPosition {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
            scale_factor: 1.0,
        };
        let _ = pos;
    }
}
