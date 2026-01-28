//! xq-app: iced UI 入口。

mod app;
mod resources;

use iced::window;
use tracing::info;

fn main() -> iced::Result {
    init_tracing();

    info!(target: "xq_app", "starting iced application");
    info!(target: "xq_app", "core: {}", xq_core::core_version());
    info!(target: "xq_app", "vision: {}", xq_vision::vision_healthcheck());
    info!(target: "xq_app", "engine: {}", xq_engine::engine_healthcheck());
    info!(target: "xq_app", "link: {}", xq_link::link_healthcheck());

    let settings = iced::Settings::default();
    let window_settings = window::Settings {
        size: (1180.0, 820.0).into(),
        ..window::Settings::default()
    };

    app::run(settings, window_settings)
}

fn init_tracing() {
    // 忽略重复初始化错误，便于未来在测试/嵌入场景复用。
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
}
