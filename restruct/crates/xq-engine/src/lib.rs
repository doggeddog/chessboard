//! xq-engine: 引擎抽象、协议适配与进程管理（实施计划 Step 3）。

mod adapter;
mod parser;
mod process;
mod profile;
mod protocol;
mod uci;
mod ucci;

pub use adapter::{create_engine, EngineAdapter, EngineBestMove, EngineEvent, EngineInfo, EngineScore};
pub use profile::{EngineOption, EngineOptions, EngineProfile, SearchParams};
pub use protocol::EngineProtocol;
pub use uci::UciAdapter;
pub use ucci::UcciAdapter;

use tracing::debug;

/// 引擎模块的最小健康检查。
pub fn engine_healthcheck() -> &'static str {
    debug!(target: "xq_engine", "engine healthcheck invoked: {}", xq_core::core_version());
    "xq-engine/ok"
}

#[cfg(test)]
mod tests {
    use super::engine_healthcheck;

    #[test]
    fn engine_healthcheck_returns_ok() {
        assert_eq!(engine_healthcheck(), "xq-engine/ok");
    }
}
