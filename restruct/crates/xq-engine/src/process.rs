//! 引擎进程管理与握手辅助。

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use tracing::{debug, warn};

use crate::adapter::EngineEvent;
use crate::parser::parse_engine_line;
use crate::protocol::EngineProtocol;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const READY_TIMEOUT: Duration = Duration::from_secs(3);

/// 运行中的引擎进程。
pub(crate) struct EngineProcess {
    protocol: EngineProtocol,
    child: Child,
    stdin: ChildStdin,
    event_rx: Receiver<EngineEvent>,
    reader_handle: Option<JoinHandle<()>>,
}

impl EngineProcess {
    /// 启动引擎进程并创建读取线程。
    pub(crate) fn spawn(path: &Path, protocol: EngineProtocol) -> Result<Self> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("启动引擎失败: {}", path.display()))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("无法获取引擎 stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("无法获取引擎 stdout"))?;

        let (tx, rx) = mpsc::channel::<EngineEvent>();
        let reader_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line_result in reader.lines() {
                match line_result {
                    Ok(line) => {
                        if let Some(event) = parse_engine_line(protocol, &line) {
                            if tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        warn!(target: "xq_engine", "读取引擎输出失败: {err}");
                        break;
                    }
                }
            }
            debug!(target: "xq_engine", "引擎输出读取线程结束");
        });

        Ok(Self {
            protocol,
            child,
            stdin,
            event_rx: rx,
            reader_handle: Some(reader_handle),
        })
    }

    /// 发送一条命令（自动追加换行并 flush）。
    pub(crate) fn send_command(&mut self, command: &str) -> Result<()> {
        debug!(target: "xq_engine", protocol = %self.protocol, "-> {command}");
        self.stdin
            .write_all(command.as_bytes())
            .with_context(|| format!("写入引擎 stdin 失败: {command}"))?;
        self.stdin
            .write_all(b"\n")
            .with_context(|| format!("写入换行失败: {command}"))?;
        self.stdin.flush().context("flush 引擎 stdin 失败")?;
        Ok(())
    }

    /// 非阻塞接收事件。
    pub(crate) fn try_recv_event(&mut self) -> Option<EngineEvent> {
        self.event_rx.try_recv().ok()
    }

    /// 带超时接收事件。
    pub(crate) fn recv_event_timeout(&mut self, timeout: Duration) -> Option<EngineEvent> {
        self.event_rx.recv_timeout(timeout).ok()
    }

    /// 执行握手流程。
    pub(crate) fn handshake(&mut self, protocol: EngineProtocol) -> Result<()> {
        match protocol {
            EngineProtocol::Uci => {
                self.send_command("uci")?;
                self.wait_for_line(HANDSHAKE_TIMEOUT, |line| line == "uciok")?;
                self.ensure_ready(protocol)?;
            }
            EngineProtocol::Ucci => {
                self.send_command("ucci")?;
                // 兼容部分引擎回显 uciok。
                self.wait_for_line(HANDSHAKE_TIMEOUT, |line| line == "ucciok" || line == "uciok")?;
                self.ensure_ready(protocol)?;
            }
        }
        Ok(())
    }

    /// 发送 isready 并等待 readyok。
    pub(crate) fn ensure_ready(&mut self, _protocol: EngineProtocol) -> Result<()> {
        self.send_command("isready")?;
        self.wait_for_line(READY_TIMEOUT, |line| line == "readyok")
    }

    fn wait_for_line<F>(&mut self, timeout: Duration, mut predicate: F) -> Result<()>
    where
        F: FnMut(&str) -> bool,
    {
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                bail!("等待引擎响应超时（{timeout:?}）");
            }
            let remaining = deadline.saturating_duration_since(now);

            match self.event_rx.recv_timeout(remaining) {
                Ok(event) => {
                    let line = event.raw_line();
                    debug!(target: "xq_engine", protocol = %self.protocol, "<- {line}");
                    if predicate(line) {
                        return Ok(());
                    }
                }
                Err(_) => bail!("等待引擎响应超时（{timeout:?}）"),
            }
        }
    }

    /// 请求引擎退出。
    pub(crate) fn quit(&mut self) -> Result<()> {
        self.send_command("quit")
    }
}

impl Drop for EngineProcess {
    fn drop(&mut self) {
        if let Err(err) = self.send_command("quit") {
            warn!(target: "xq_engine", "drop 时发送 quit 失败: {err}");
        }

        match self.child.try_wait() {
            Ok(Some(_status)) => {}
            Ok(None) => {
                if let Err(err) = self.child.kill() {
                    warn!(target: "xq_engine", "杀死引擎进程失败: {err}");
                }
            }
            Err(err) => {
                warn!(target: "xq_engine", "检查引擎进程状态失败: {err}");
            }
        }

        let _ = self.child.wait();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}
