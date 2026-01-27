//! xq-link: external window linking and sync helpers.

use tracing::debug;

mod geometry;
mod inject;
mod permission;
mod runtime;
mod sync;
mod window;

pub use geometry::{BoardGeometry, ScreenPoint};
pub use inject::{EnigoInjector, InputInjector, InputPlan};
pub use permission::{check_input_permission, InputPermissionStatus, PermissionCheck};
pub use runtime::{InjectResult, LinkRuntime, LinkRuntimeConfig, LinkStep};
pub use sync::{
    AlignmentStatus, DesyncReason, ExternalUpdate, PendingInjection, SyncConfig, SyncPolicy,
    SyncState,
};
pub use window::{list_windows, LinkWindow, LinkWindowInfo, WindowPosition};

/// 连线模块的最小健康检查。
pub fn link_healthcheck() -> &'static str {
    debug!(target: "xq_link", "vision: {}", xq_vision::vision_healthcheck());
    let _ = xq_core::core_version();
    "xq-link/ok"
}

#[cfg(test)]
mod tests {
    use super::link_healthcheck;

    #[test]
    fn link_healthcheck_returns_ok() {
        assert_eq!(link_healthcheck(), "xq-link/ok");
    }
}
