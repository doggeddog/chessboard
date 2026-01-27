//! UCCI 协议适配器（最小可用实现）。

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::adapter::{init_and_apply_profile, EngineAdapter, EngineEvent};
use crate::process::EngineProcess;
use crate::profile::{EngineProfile, SearchParams};
use crate::protocol::EngineProtocol;

/// UCCI 协议适配器。
pub struct UcciAdapter {
    process: EngineProcess,
}

impl UcciAdapter {
    /// 启动一个 UCCI 引擎进程。
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let process = EngineProcess::spawn(path.as_ref(), EngineProtocol::Ucci)
            .with_context(|| format!("启动 UCCI 引擎失败: {}", path.as_ref().display()))?;
        Ok(Self { process })
    }
}

impl EngineAdapter for UcciAdapter {
    fn protocol(&self) -> EngineProtocol {
        EngineProtocol::Ucci
    }

    fn init(&mut self) -> Result<()> {
        self.process.handshake(EngineProtocol::Ucci)
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let cmd = format!("setoption name {name} value {value}");
        self.process.send_command(&cmd)?;
        self.process.ensure_ready(EngineProtocol::Ucci)
    }

    fn apply_profile(&mut self, profile: &EngineProfile) -> Result<()> {
        if profile.protocol != EngineProtocol::Ucci {
            bail!("profile 协议不匹配: 期望 ucci, 实际 {}", profile.protocol);
        }
        init_and_apply_profile(&mut self.process, EngineProtocol::Ucci, profile)
    }

    fn position_fen(&mut self, fen: &str) -> Result<()> {
        let cmd = format!("position fen {fen}");
        self.process.send_command(&cmd)
    }

    fn go(&mut self, params: &SearchParams) -> Result<()> {
        let cmd = params.to_go_command(EngineProtocol::Ucci);
        self.process.send_command(&cmd)
    }

    fn stop(&mut self) -> Result<()> {
        self.process.send_command("stop")
    }

    fn quit(&mut self) -> Result<()> {
        self.process.quit()
    }

    fn try_recv_event(&mut self) -> Option<EngineEvent> {
        self.process.try_recv_event()
    }

    fn recv_event_timeout(&mut self, timeout: Duration) -> Option<EngineEvent> {
        self.process.recv_event_timeout(timeout)
    }
}
