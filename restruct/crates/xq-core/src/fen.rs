use thiserror::Error;

use crate::types::{Board, BoardCell, Piece, Pos, Side, BOARD_COLS, BOARD_ROWS};

/// FEN 解析配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FenParseOptions {
    /// 是否要求提供行棋方字段。
    pub require_side: bool,
    /// 是否允许出现额外字段（会被忽略）。
    pub allow_extra_fields: bool,
}

impl FenParseOptions {
    /// 严格模式：要求 side 字段，允许额外字段但忽略。
    #[must_use]
    pub const fn strict() -> Self {
        Self {
            require_side: true,
            allow_extra_fields: true,
        }
    }
}

impl Default for FenParseOptions {
    fn default() -> Self {
        Self::strict()
    }
}

/// FEN 解析错误。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FenError {
    #[error("FEN 为空")]
    Empty,
    #[error("FEN 行数必须为 10，实际为 {0}")]
    InvalidRowCount(usize),
    #[error("第 {row} 行列数必须为 9，实际为 {cols}")]
    InvalidColCount { row: usize, cols: usize },
    #[error("第 {row} 行出现非法字符: {ch}")]
    InvalidChar { row: usize, ch: char },
    #[error("缺少行棋方字段")]
    MissingSide,
    #[error("非法行棋方字段: {0}")]
    InvalidSide(String),
    #[error("FEN 包含额外字段但当前配置不允许: {0}")]
    ExtraFields(String),
}

/// 解析中象 FEN，返回 `Board`。
pub fn parse_fen(fen: &str, options: FenParseOptions) -> Result<Board, FenError> {
    let trimmed = fen.trim();
    if trimmed.is_empty() {
        return Err(FenError::Empty);
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return Err(FenError::Empty);
    }

    if !options.allow_extra_fields && parts.len() > 2 {
        return Err(FenError::ExtraFields(parts[2..].join(" ")));
    }

    let board_part = parts[0];
    let side = match parts.get(1) {
        Some(side_part) => parse_side(side_part)?,
        None => {
            if options.require_side {
                return Err(FenError::MissingSide);
            }
            Side::Red
        }
    };

    let grid = parse_board_grid(board_part)?;
    Ok(Board {
        grid,
        side_to_move: side,
    })
}

fn parse_side(raw: &str) -> Result<Side, FenError> {
    let mut chars = raw.chars();
    let first = chars.next().ok_or_else(|| FenError::InvalidSide(raw.to_string()))?;
    Side::from_fen_char(first).ok_or_else(|| FenError::InvalidSide(raw.to_string()))
}

fn parse_board_grid(board_part: &str) -> Result<[[BoardCell; BOARD_COLS]; BOARD_ROWS], FenError> {
    let rows: Vec<&str> = board_part.split('/').collect();
    if rows.len() != BOARD_ROWS {
        return Err(FenError::InvalidRowCount(rows.len()));
    }

    let mut grid = [[None; BOARD_COLS]; BOARD_ROWS];

    for (row_idx, row_str) in rows.iter().enumerate() {
        let mut col_idx = 0usize;
        let mut digit_buf = 0usize;

        let flush_digits = |digit_buf: &mut usize, col_idx: &mut usize| {
            if *digit_buf > 0 {
                *col_idx += *digit_buf;
                *digit_buf = 0;
            }
        };

        for ch in row_str.chars() {
            if ch.is_ascii_digit() {
                let digit = (ch as u8 - b'0') as usize;
                digit_buf = digit_buf * 10 + digit;
                continue;
            }

            flush_digits(&mut digit_buf, &mut col_idx);

            let piece = Piece::from_fen_char(ch)
                .ok_or(FenError::InvalidChar { row: row_idx, ch })?;

            if col_idx >= BOARD_COLS {
                return Err(FenError::InvalidColCount {
                    row: row_idx,
                    cols: col_idx + 1,
                });
            }

            grid[row_idx][col_idx] = Some(piece);
            col_idx += 1;
        }

        flush_digits(&mut digit_buf, &mut col_idx);

        if col_idx != BOARD_COLS {
            return Err(FenError::InvalidColCount {
                row: row_idx,
                cols: col_idx,
            });
        }
    }

    Ok(grid)
}

impl Board {
    /// 从 FEN 创建棋盘（严格模式）。
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        parse_fen(fen, FenParseOptions::strict())
    }

    /// 生成规范化的 FEN（仅包含棋盘与行棋方）。
    #[must_use]
    pub fn to_fen(&self) -> String {
        let mut out = String::new();

        for row in 0..BOARD_ROWS {
            let mut empty = 0usize;

            for col in 0..BOARD_COLS {
                match self.grid[row][col] {
                    Some(piece) => {
                        if empty > 0 {
                            out.push_str(&empty.to_string());
                            empty = 0;
                        }
                        out.push(piece.to_fen_char());
                    }
                    None => empty += 1,
                }
            }

            if empty > 0 {
                out.push_str(&empty.to_string());
            }

            if row + 1 != BOARD_ROWS {
                out.push('/');
            }
        }

        out.push(' ');
        out.push(self.side_to_move.to_fen_char());
        out
    }

    /// 便捷方法：将棋盘索引位置转换为 ICCS 坐标（调试与测试用）。
    #[must_use]
    pub fn pos_from_index(row: usize, col: usize) -> Option<Pos> {
        Pos::from_index(row, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::STARTPOS_FEN;

    fn round_trip(fen: &str) {
        let board = Board::from_fen(fen).expect("FEN 应可解析");
        let fen_back = board.to_fen();

        // 以“解析后再生成”的规范化结果作为基准，保证互逆等价。
        let normalized = Board::from_fen(&fen_back)
            .expect("规范化 FEN 应可解析")
            .to_fen();
        assert_eq!(fen_back, normalized);
    }

    #[test]
    fn startpos_round_trip() {
        round_trip(STARTPOS_FEN);
    }

    #[test]
    fn fen_round_trip_examples() {
        // 覆盖开局、中局、残棋、空格压缩边界等场景（>=10 组）。
        let samples = [
            STARTPOS_FEN,
            "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR b",
            "r1bakab1r/4n4/1c5c1/p1p1p1p1p/9/4P4/P1P3P1P/1C5C1/4N4/RNBAKAB1R w",
            "2bakab2/9/4c4/9/9/9/4C4/9/9/2BAKAB2 w",
            "4k4/9/9/9/9/9/9/9/9/4K4 b",
            "9/9/9/9/9/9/9/9/9/9 w",
            "r8/9/9/9/9/9/9/9/9/R8 w",
            "3ak4/4a4/9/9/9/9/9/9/4A4/3AK4 b",
            "2b1k1b2/9/9/9/9/9/9/9/9/2B1K1B2 w",
            "4k4/9/9/9/4r4/9/9/9/9/4K4 w",
            // 含额外字段：解析后会规范化为仅两段。
            "4k4/9/9/9/9/9/9/9/9/4K4 w - - 0 1",
        ];

        for fen in samples {
            round_trip(fen);
        }
    }

    #[test]
    fn fen_piece_count_matches_startpos() {
        let board = Board::from_fen(STARTPOS_FEN).expect("startpos FEN");
        assert_eq!(board.piece_count(), 32);
    }

    #[test]
    fn invalid_row_count_is_rejected() {
        let err = Board::from_fen("9/9/9/9/9/9/9/9/9 w").expect_err("应拒绝 9 行");
        assert!(matches!(err, FenError::InvalidRowCount(9)));
    }
}
