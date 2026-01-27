//! UCI 协议适配器（最小可用实现）。

use std::path::Path;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::adapter::{init_and_apply_profile, EngineAdapter, EngineEvent};
use crate::process::EngineProcess;
use crate::profile::{EngineProfile, SearchParams};
use crate::protocol::EngineProtocol;

/// UCI 协议适配器。
pub struct UciAdapter {
    process: EngineProcess,
}

impl UciAdapter {
    /// 启动一个 UCI 引擎进程。
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let process = EngineProcess::spawn(path.as_ref(), EngineProtocol::Uci)
            .with_context(|| format!("启动 UCI 引擎失败: {}", path.as_ref().display()))?;
        Ok(Self { process })
    }
}

impl EngineAdapter for UciAdapter {
    fn protocol(&self) -> EngineProtocol {
        EngineProtocol::Uci
    }

    fn init(&mut self) -> Result<()> {
        self.process.handshake(EngineProtocol::Uci)
    }

    fn set_option(&mut self, name: &str, value: &str) -> Result<()> {
        let cmd = format!("setoption name {name} value {value}");
        self.process.send_command(&cmd)?;
        // setoption 后通常需要一次 isready 确认。
        self.process.ensure_ready(EngineProtocol::Uci)
    }

    fn apply_profile(&mut self, profile: &EngineProfile) -> Result<()> {
        if profile.protocol != EngineProtocol::Uci {
            bail!("profile 协议不匹配: 期望 uci, 实际 {}", profile.protocol);
        }
        init_and_apply_profile(&mut self.process, EngineProtocol::Uci, profile)
    }

    fn position_fen(&mut self, fen: &str) -> Result<()> {
        let cmd = format!("position fen {fen}");
        self.process.send_command(&cmd)
    }

    fn go(&mut self, params: &SearchParams) -> Result<()> {
        let cmd = params.to_go_command(EngineProtocol::Uci);
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

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use anyhow::{Context, Result};
    use xq_core::STARTPOS_FEN;

    use crate::adapter::{create_engine, EngineEvent};
    use crate::profile::{EngineProfile, SearchParams};
    use crate::protocol::EngineProtocol;

    fn unique_script_path() -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("时间应递增")
            .as_millis();
        std::env::temp_dir().join(format!("xq-engine-fake-uci-{ts}.sh"))
    }

    fn write_fake_uci_engine() -> Result<PathBuf> {
        let path = unique_script_path();
        let mut file = fs::File::create(&path)
            .with_context(|| format!("创建假引擎脚本失败: {}", path.display()))?;

        // 一个最小可用的 UCI 假引擎：响应握手、返回固定 info 与 bestmove。
        let script = r#"#!/bin/sh
while IFS= read -r line; do
  case "$line" in
    uci)
      echo "id name fake-uci"
      echo "uciok"
      ;;
    isready)
      echo "readyok"
      ;;
    position*)
      ;;
    go*)
      echo "info depth 4 score cp 12 time 5 nodes 128 pv h2e2 h9e9"
      echo "bestmove h2e2"
      ;;
    stop)
      ;;
    quit)
      exit 0
      ;;
    *)
      ;;
  esac
done
"#;

        file.write_all(script.as_bytes())?;
        file.flush()?;

        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms)?;

        Ok(path)
    }

    #[test]
    fn uci_adapter_can_handshake_and_receive_bestmove() -> Result<()> {
        let engine_path = write_fake_uci_engine()?;

        let profile = EngineProfile::new("fake", EngineProtocol::Uci, &engine_path).with_search_depth(4);
        let mut engine = create_engine(&profile)?;

        engine.init()?;
        engine.apply_profile(&profile)?;
        engine.position_fen(STARTPOS_FEN)?;
        engine.go(&SearchParams::with_depth(4))?;

        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut got_bestmove = false;
        while std::time::Instant::now() < deadline {
            if let Some(event) = engine.recv_event_timeout(Duration::from_millis(200)) {
                if let EngineEvent::BestMove(bm) = event {
                    got_bestmove = bm.bestmove.is_some();
                    break;
                }
            }
        }

        engine.quit()?;
        let _ = fs::remove_file(engine_path);

        assert!(got_bestmove, "应在超时前收到 bestmove");
        Ok(())
    }
}
