use crate::types::{Board, Move, Pos, BOARD_COLS, BOARD_ROWS};

/// 180° 翻转一个坐标（用于棋盘反转）。
#[must_use]
pub const fn flip_pos(pos: Pos) -> Pos {
    let file = BOARD_COLS as u8 - 1 - pos.file;
    let rank = BOARD_ROWS as u8 - 1 - pos.rank;
    Pos { file, rank }
}

/// 180° 翻转一个走法（from/to 同时翻转）。
#[must_use]
pub const fn flip_move(mv: Move) -> Move {
    Move {
        from: flip_pos(mv.from),
        to: flip_pos(mv.to),
    }
}

/// 根据是否翻转进行坐标映射。
#[must_use]
pub const fn map_pos_with_flip(pos: Pos, flipped: bool) -> Pos {
    if flipped {
        flip_pos(pos)
    } else {
        pos
    }
}

impl Board {
    /// 返回一个 180° 翻转后的棋盘（棋子阵营不变，仅坐标变换）。
    #[must_use]
    pub fn flipped(&self) -> Self {
        let mut grid = [[None; BOARD_COLS]; BOARD_ROWS];

        for row in 0..BOARD_ROWS {
            for col in 0..BOARD_COLS {
                let target_row = BOARD_ROWS - 1 - row;
                let target_col = BOARD_COLS - 1 - col;
                grid[target_row][target_col] = self.grid[row][col];
            }
        }

        Self {
            grid,
            side_to_move: self.side_to_move,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Board, Move, Pos, Side};

    fn must_pos(s: &str) -> Pos {
        Pos::from_iccs_square(s).expect("合法坐标")
    }

    #[test]
    fn double_flip_pos_is_identity() {
        let samples = [
            "a0", "e0", "i0", "a4", "e4", "i4", "a5", "e5", "i5", "a9", "e9",
            "i9", "b2", "c3", "d6", "f7", "g1", "h8", "c8", "h1",
        ];

        for s in samples {
            let pos = must_pos(s);
            assert_eq!(flip_pos(flip_pos(pos)), pos, "坐标 {s} 翻转两次应还原");
        }
    }

    #[test]
    fn double_flip_move_is_identity() {
        let samples = [
            "a0a1", "a0a9", "e2e7", "b2c4", "h2e2", "c0e2", "i9i8", "d0e1",
            "e1e2", "g3g4",
        ];

        for s in samples {
            let mv = Move::from_iccs(s).expect("合法走法");
            assert_eq!(flip_move(flip_move(mv)), mv, "走法 {s} 翻转两次应还原");
        }
    }

    #[test]
    fn board_flip_twice_is_identity() {
        let board = Board::startpos();
        let flipped_twice = board.flipped().flipped();
        assert_eq!(board, flipped_twice);
    }

    #[test]
    fn map_pos_with_flip_respects_flag() {
        let pos = must_pos("b2");
        assert_eq!(map_pos_with_flip(pos, false), pos);
        assert_eq!(map_pos_with_flip(pos, true), flip_pos(pos));
    }

    #[test]
    fn flip_preserves_side_to_move() {
        let board = Board::empty(Side::Black);
        assert_eq!(board.flipped().side_to_move, Side::Black);
    }
}
