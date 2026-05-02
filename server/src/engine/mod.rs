pub mod chessdb;
use std::fmt::Display;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
mod command;

use tracing::debug;
use tracing::trace;

use crate::chess;

#[derive(Debug, serde::Serialize, Default, Clone)]
pub struct QueryResult {
    pub depth: usize,              // 深度
    pub score: isize,              // 得分
    pub time: usize,               // 时间(ms)
    pub nodes: u64,                // 搜索节点数
    pub nps: u64,                  // 每秒节点数
    pub wdl: Option<[u16; 3]>,     // 胜/和/负概率 (per mille)
    pub pvs: Vec<String>,          // 思考(iccs)
    pub moves: Vec<String>,        // 思考(chinese)
    pub state: QueryState,         // 状态
    pub source: String,            // 来源
}

const SOURCE_ENGINE: &str = "引擎";

#[derive(Debug, serde::Serialize, Default, Clone, Copy)]
pub enum QueryState {
    Success,
    #[default]
    NotResult,
    InvalidBoard,
    ServerInternalError, // 内部错误
}

#[derive(Debug, serde::Serialize, Clone, serde::Deserialize, Copy)]
pub struct EngineConfig {
    pub depth: usize,
    pub time: usize,
    pub threads: usize,
    pub hash: usize,
    pub show_wdl: bool,
    pub chessdb_enabled: bool,
    pub chessdb_timeout: u64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self { depth: 20, time: 5000, threads: 4, hash: 64, show_wdl: false, chessdb_enabled: true, chessdb_timeout: 5 }
    }
}

pub struct Engine {
    stdin: Box<dyn Write>,
    stdout: Box<dyn BufRead>,
    child: std::process::Child, // 添加子进程字段
}

unsafe impl Send for Engine {}
unsafe impl Sync for Engine {}

impl Engine {
    pub fn new(libs: &Path) -> Self {
        let mut child = command::new(libs);

        let nnue = libs.join("pikafish.nnue");

        let stdin = Box::new(child.stdin.take().unwrap());
        let stdout = Box::new(BufReader::new(child.stdout.take().unwrap()));

        let mut eng = Engine { stdin, stdout, child };
        eng.setoption("EvalFile", nnue.display());
        eng.setoption("Sixty Move Rule", false);
        eng
    }

    pub fn reload(&mut self, libs: &Path, config: &EngineConfig) {
        self.child.kill().unwrap();
        self.child.wait().unwrap();
        *self = Self::new(libs);
        self.set_hash(config.hash);
        self.set_show_wdl(config.show_wdl);
        self.set_threads(config.threads);
    }

    fn write_command<A: Display>(&mut self, args: A) {
        writeln!(self.stdin, "{}", args).expect("write command error");
        self.stdin.flush().expect("write command flush error");
        debug!("{}", args);
    }

    pub fn set_show_wdl(&mut self, open: bool) { self.setoption("UCI_ShowWDL", open); }

    pub fn set_threads(&mut self, num: usize) { self.setoption("Threads", num); }

    pub fn set_hash(&mut self, size: usize) { self.setoption("Hash", size); }

    pub fn setoption<T: Display>(&mut self, name: &str, value: T) {
        self.write_command(format!("setoption name {} value {}", name, value))
    }

    pub fn position(&mut self, fen: &str) { self.write_command(format!("position fen {}", fen)) }

    fn read_line(&mut self) -> String {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        trace!("line::{}", line);
        line.trim().to_string()
    }

    fn parse_line(&self, line: String, result: &mut QueryResult) {
        let mut iter = line.split_whitespace();
        result.source = SOURCE_ENGINE.to_string();
        loop {
            if let Some(key) = iter.next() {
                match key {
                    "depth" => {
                        result.depth = iter.next().unwrap().parse().unwrap();
                    }
                    "time" => {
                        result.time = iter.next().unwrap().parse().unwrap();
                    }
                    "nodes" => {
                        result.nodes = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                    "nps" => {
                        result.nps = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                    }
                    "score" => match iter.next().unwrap() {
                        "cp" => {
                            result.score = iter.next().unwrap().parse().unwrap();
                        }
                        "mate" => {
                            let round: isize = iter.next().unwrap().parse().unwrap();
                            result.score = if round > 0 { 30000 - round } else { -(30000 + round) };
                        }
                        _ => {}
                    },
                    "wdl" => {
                        let w = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        let d = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        let l = iter.next().and_then(|v| v.parse().ok()).unwrap_or(0);
                        result.wdl = Some([w, d, l]);
                    }
                    "pv" => loop {
                        if let Some(pv) = iter.next() {
                            result.pvs.push(pv.to_string());
                            continue;
                        }
                        break;
                    },
                    _ => {}
                }
                continue;
            }
            break;
        }
    }

    /// 流式引擎搜索：每收到一行 info 就调用 on_info 回调，
    /// board 用于将 PV 的 ICCS 走法翻译为中文。
    pub fn go_streaming<F>(
        &mut self,
        depth: usize,
        time: usize,
        board: [[char; 9]; 10],
        mut on_info: F,
    ) -> QueryResult
    where
        F: FnMut(&QueryResult),
    {
        self.write_command(format!("go depth {} movetime {}", depth, time));
        let mut final_result = QueryResult::default();
        final_result.source = SOURCE_ENGINE.to_string();

        loop {
            let line = self.read_line();
            if line.starts_with("bestmove") {
                trace!("{}", line);
                break;
            }
            if line.starts_with("info") && line.contains(" pv ") {
                let mut result = QueryResult::default();
                self.parse_line(line, &mut result);

                let mut tmp_board = board;
                for pv in &result.pvs {
                    if let Some(chinese) = Self::try_translate_pv(tmp_board, pv) {
                        result.moves.push(chinese);
                        tmp_board = chess::board_move(tmp_board, pv);
                    } else {
                        break;
                    }
                }

                on_info(&result);
                final_result = result;
            }
        }

        final_result
    }

    fn try_translate_pv(board: [[char; 9]; 10], pv: &str) -> Option<String> {
        if pv.len() != 4 {
            return None;
        }
        let mv = chess::Move::new(pv);
        if mv.from_y >= 10 || mv.from_x >= 9 || mv.to_y >= 10 || mv.to_x >= 9 {
            return None;
        }
        if board[mv.from_y][mv.from_x] == ' ' {
            return None;
        }
        Some(chess::board_move_chinese(board, pv))
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.write_command("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use std::path;

    use tracing::info;
    use tracing::Level;

    use super::*;
    use crate::logger;

    #[tokio::test]
    async fn test_query() {
        logger::init_tracer(Level::TRACE, &std::path::PathBuf::from("."));
        let fen = "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C2C4/9/RNBAKABNR b";
        let result = chessdb::query(fen, 10).await;
        info!("{:?}", result);
    }

    #[test]
    fn test_engine() {
        logger::init_tracer(Level::TRACE, &std::path::PathBuf::from("."));
        let fen = "4k4/9/6r2/9/9/9/9/9/4A4/4K4 w";
        let libs = path::PathBuf::from("/Users/atopx/script/chessboard/libs");
        let mut eng = Engine::new(&libs);
        let board = chess::fen_to_board(fen);
        eng.position(fen);
        let result = eng.go_streaming(20, 5000, board, |info| {
            info!("depth={} score={} nps={}", info.depth, info.score, info.nps);
        });
        info!("{:?}", result);
    }
}
