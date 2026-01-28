use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use anyhow::{Context, Result};
use xq_core::Move;

use crate::adapter::{EngineInfo, EngineScore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChessDbResponse {
    NotResult,
    InvalidBoard(String),
    Hit(ChessDbHit),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChessDbHit {
    pub score: Option<i32>,
    pub depth: Option<u32>,
    pub pv: Vec<Move>,
    pub pv_raw: Vec<String>,
    pub raw: String,
}

impl ChessDbHit {
    #[must_use]
    pub fn to_engine_info(&self) -> EngineInfo {
        let score = self.score.map(EngineScore::Cp);
        EngineInfo {
            depth: self.depth,
            score,
            pv: self.pv.clone(),
            pv_raw: self.pv_raw.clone(),
            raw: self.raw.clone(),
            ..EngineInfo::default()
        }
    }
}

pub fn query_chessdb(fen: &str, timeout: Duration) -> Result<ChessDbResponse> {
    let host = "www.chessdb.cn";
    let request = build_request(host, fen);

    let addr = (host, 80)
        .to_socket_addrs()
        .context("解析 chessdb 地址失败")?
        .next()
        .context("chessdb 地址为空")?;

    let mut stream = TcpStream::connect_timeout(&addr, timeout)
        .context("连接 chessdb 失败")?;
    stream
        .set_read_timeout(Some(timeout))
        .context("设置读取超时失败")?;
    stream
        .set_write_timeout(Some(timeout))
        .context("设置写入超时失败")?;

    stream
        .write_all(request.as_bytes())
        .context("发送 chessdb 请求失败")?;
    stream.flush().ok();

    let mut buf = Vec::new();
    stream
        .read_to_end(&mut buf)
        .context("读取 chessdb 响应失败")?;

    let raw = String::from_utf8_lossy(&buf);
    let body = raw
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.trim())
        .unwrap_or(raw.trim());

    parse_body(body)
}

fn build_request(host: &str, fen: &str) -> String {
    let encoded_fen = url_encode(fen);
    format!(
        "GET /chessdb.php?action=querypv&board={encoded_fen} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: xq-app/0.1\r\nReferer: https://www.chessdb.cn/query/\r\nConnection: close\r\n\r\n"
    )
}

fn parse_body(body: &str) -> Result<ChessDbResponse> {
    let content = body.trim();
    if content.is_empty() || content.eq_ignore_ascii_case("unknown") {
        return Ok(ChessDbResponse::NotResult);
    }

    let lower = content.to_ascii_lowercase();
    if matches!(lower.as_str(), "invalid board" | "checkmate" | "stalemate") {
        return Ok(ChessDbResponse::InvalidBoard(content.to_string()));
    }

    let mut hit = ChessDbHit {
        raw: content.to_string(),
        ..ChessDbHit::default()
    };

    for part in content.split(',') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix("score:") {
            hit.score = value.trim().parse::<i32>().ok();
            continue;
        }
        if let Some(value) = part.strip_prefix("depth:") {
            hit.depth = value.trim().parse::<u32>().ok();
            continue;
        }
        if let Some(value) = part.strip_prefix("pv:") {
            let pv_raw: Vec<String> = value
                .split('|')
                .map(|mv| mv.trim().to_string())
                .filter(|mv| !mv.is_empty())
                .collect();
            hit.pv_raw = pv_raw.clone();
            hit.pv = pv_raw
                .iter()
                .filter_map(|mv| Move::from_iccs(mv).ok())
                .collect();
        }
    }

    Ok(ChessDbResponse::Hit(hit))
}

fn url_encode(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        let ch = b as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::url_encode;

    #[test]
    fn url_encode_preserves_safe_chars() {
        assert_eq!(url_encode("abcXYZ-_.~"), "abcXYZ-_.~");
    }

    #[test]
    fn url_encode_encodes_spaces() {
        assert_eq!(url_encode("a b"), "a%20b");
    }
}
