//! 引擎输出解析（info / bestmove / 其他原始行）。

use xq_core::Move;

use crate::{adapter::{EngineBestMove, EngineEvent, EngineInfo, EngineScore}, protocol::EngineProtocol};

/// 解析一行引擎输出为结构化事件。
#[must_use]
pub fn parse_engine_line(protocol: EngineProtocol, line: &str) -> Option<EngineEvent> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("info") {
        return Some(EngineEvent::Info(parse_info_line(protocol, trimmed)));
    }

    if trimmed.starts_with("bestmove") {
        return Some(EngineEvent::BestMove(parse_bestmove_line(trimmed)));
    }

    Some(EngineEvent::RawLine(trimmed.to_string()))
}

fn parse_info_line(_protocol: EngineProtocol, line: &str) -> EngineInfo {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut info = EngineInfo {
        raw: line.to_string(),
        ..EngineInfo::default()
    };

    let mut i = 1; // 跳过 "info"
    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u32>().ok()) {
                    info.depth = Some(value);
                }
                i += 2;
            }
            "seldepth" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u32>().ok()) {
                    info.seldepth = Some(value);
                }
                i += 2;
            }
            "time" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                    info.time_ms = Some(value);
                }
                i += 2;
            }
            "nodes" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                    info.nodes = Some(value);
                }
                i += 2;
            }
            "nps" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u64>().ok()) {
                    info.nps = Some(value);
                }
                i += 2;
            }
            "multipv" => {
                if let Some(value) = tokens.get(i + 1).and_then(|v| v.parse::<u32>().ok()) {
                    info.multipv = Some(value);
                }
                i += 2;
            }
            "score" => {
                let kind = tokens.get(i + 1).copied();
                let value = tokens.get(i + 2).copied();
                match (kind, value) {
                    (Some("cp"), Some(v)) => {
                        if let Ok(cp) = v.parse::<i32>() {
                            info.score = Some(EngineScore::Cp(cp));
                        }
                    }
                    (Some("mate"), Some(v)) => {
                        if let Ok(mate) = v.parse::<i32>() {
                            info.score = Some(EngineScore::Mate(mate));
                        }
                    }
                    _ => {}
                }
                i += 3;
            }
            "pv" => {
                let pv_tokens = &tokens[(i + 1)..];
                info.pv_raw = pv_tokens.iter().map(|s| (*s).to_string()).collect();
                info.pv = pv_tokens.iter().filter_map(|t| parse_move_token(t)).collect();
                break;
            }
            _ => {
                i += 1;
            }
        }
    }

    info
}

fn parse_bestmove_line(line: &str) -> EngineBestMove {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    let bestmove_raw = tokens.get(1).copied().unwrap_or("(none)").to_string();
    let bestmove = parse_move_token(&bestmove_raw);

    let mut ponder = None;
    let mut idx = 2;
    while idx + 1 < tokens.len() {
        if tokens[idx] == "ponder" {
            ponder = parse_move_token(tokens[idx + 1]);
            break;
        }
        idx += 1;
    }

    EngineBestMove {
        bestmove,
        bestmove_raw,
        ponder,
        raw: line.to_string(),
    }
}

fn parse_move_token(token: &str) -> Option<Move> {
    // 兼容常见“无走法”占位。
    if token == "0000" || token.eq_ignore_ascii_case("(none)") {
        return None;
    }

    Move::from_iccs(token).ok()
}

#[cfg(test)]
mod tests {
    use crate::adapter::{EngineEvent, EngineScore};
    use crate::protocol::EngineProtocol;

    use super::parse_engine_line;

    #[test]
    fn parse_info_extracts_score_and_pv() {
        let line = "info depth 12 score cp 34 time 120 nodes 4096 pv h2e2 h9e9";
        let event = parse_engine_line(EngineProtocol::Uci, line).expect("应能解析");

        match event {
            EngineEvent::Info(info) => {
                assert_eq!(info.depth, Some(12));
                assert_eq!(info.time_ms, Some(120));
                assert_eq!(info.nodes, Some(4096));
                assert_eq!(info.score, Some(EngineScore::Cp(34)));
                assert_eq!(info.pv_raw.len(), 2);
                assert_eq!(info.pv.len(), 2);
            }
            _ => panic!("期望 info 事件"),
        }
    }

    #[test]
    fn parse_bestmove_handles_none() {
        let event = parse_engine_line(EngineProtocol::Uci, "bestmove 0000").expect("应能解析");
        match event {
            EngineEvent::BestMove(bm) => {
                assert!(bm.bestmove.is_none());
                assert_eq!(bm.bestmove_raw, "0000");
            }
            _ => panic!("期望 bestmove 事件"),
        }
    }
}
