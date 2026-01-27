//! 统一引擎抽象接口与事件模型。

use std::time::Duration;

use anyhow::{bail, Context, Result};
use xq_core::Move;

use crate::{process::EngineProcess, profile::EngineProfile, protocol::EngineProtocol, uci::UciAdapter, ucci::UcciAdapter};

/// 引擎分数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineScore {
    Cp(i32),
    Mate(i32),
}

/// 引擎 info 事件（结构化）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineInfo {
    pub depth: Option<u32>,
    pub seldepth: Option<u32>,
    pub time_ms: Option<u64>,
    pub nodes: Option<u64>,
    pub nps: Option<u64>,
    pub multipv: Option<u32>,
    pub score: Option<EngineScore>,
    pub pv: Vec<Move>,
    pub pv_raw: Vec<String>,
    pub raw: String,
}

/// 引擎 bestmove 事件（结构化）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineBestMove {
    pub bestmove: Option<Move>,
    pub bestmove_raw: String,
    pub ponder: Option<Move>,
    pub raw: String,
}

/// 引擎输出事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Info(EngineInfo),
    BestMove(EngineBestMove),
    RawLine(String),
}

impl EngineEvent {
    /// 获取原始行文本（用于日志/调试）。
    #[must_use]
    pub fn raw_line(&self) -> &str {
        match self {
            Self::Info(info) => &info.raw,
            Self::BestMove(bm) => &bm.raw,
            Self::RawLine(line) => line,
        }
    }
}

/// 统一引擎适配器接口。
///
/// 设计目标：上层只依赖该 trait，不依赖具体协议实现细节。
pub trait EngineAdapter: Send {
    /// 返回协议类型。
    fn protocol(&self) -> EngineProtocol;

    /// 初始化握手（uci/ucci + isready）。
    fn init(&mut self) -> Result<()>;

    /// 设置单个 option。
    fn set_option(&mut self, name: &str, value: &str) -> Result<()>;

    /// 应用 profile（批量 setoption）。
    fn apply_profile(&mut self, profile: &EngineProfile) -> Result<()>;

    /// 设置局面（FEN）。
    fn position_fen(&mut self, fen: &str) -> Result<()>;

    /// 开始搜索。
    fn go(&mut self, params: &crate::profile::SearchParams) -> Result<()>;

    /// 停止搜索。
    fn stop(&mut self) -> Result<()>;

    /// 退出引擎进程。
    fn quit(&mut self) -> Result<()>;

    /// 非阻塞地尝试接收一个事件。
    fn try_recv_event(&mut self) -> Option<EngineEvent>;

    /// 在指定超时内阻塞接收事件。
    fn recv_event_timeout(&mut self, timeout: Duration) -> Option<EngineEvent>;
}

/// 根据 profile 创建对应协议的引擎适配器。
pub fn create_engine(profile: &EngineProfile) -> Result<Box<dyn EngineAdapter>> {
    if !profile.engine_path.exists() {
        bail!("引擎路径不存在: {}", profile.engine_path.display());
    }

    let adapter: Box<dyn EngineAdapter> = match profile.protocol {
        EngineProtocol::Uci => Box::new(UciAdapter::new(&profile.engine_path)?),
        EngineProtocol::Ucci => Box::new(UcciAdapter::new(&profile.engine_path)?),
    };

    Ok(adapter)
}

/// 适配器内部公用：初始化 + 应用 profile 的标准流程。
pub(crate) fn init_and_apply_profile(
    process: &mut EngineProcess,
    protocol: EngineProtocol,
    profile: &EngineProfile,
) -> Result<()> {
    process
        .handshake(protocol)
        .with_context(|| format!("{protocol} 握手失败"))?;

    for cmd in profile.to_setoption_commands() {
        process
            .send_command(&cmd)
            .with_context(|| format!("发送 setoption 失败: {cmd}"))?;
    }

    // 额外做一次 ready 检查，确保 option 已生效。
    process.ensure_ready(protocol)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::EngineEvent;

    #[test]
    fn raw_line_accessor_is_stable() {
        let evt = EngineEvent::RawLine("readyok".to_string());
        assert_eq!(evt.raw_line(), "readyok");
    }
}
