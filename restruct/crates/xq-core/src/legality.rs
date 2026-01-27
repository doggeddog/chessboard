use thiserror::Error;

use crate::types::{Board, Move, Piece, PieceKind, Pos, Side, BOARD_COLS, BOARD_ROWS};

/// 走法合法性检查结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveLegality {
    Legal,
    Illegal(LegalMoveError),
}

/// 走法不合法的原因。
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LegalMoveError {
    #[error("坐标超出棋盘范围")]
    OutOfBounds,
    #[error("起点没有棋子")]
    NoPieceAtFrom,
    #[error("行棋方不匹配：期望 {expected:?}，实际 {found:?}")]
    WrongSideToMove { expected: Side, found: Side },
    #[error("目标位置有己方棋子")]
    FriendlyFire,
    #[error("不符合该棋子的移动规则：{0:?}")]
    InvalidPieceRule(PieceKind),
    #[error("走子后将帅照面")]
    KingsFaceEachOther,
}

/// 对外暴露的合法性检查入口。
#[must_use]
pub fn check_move_legality(board: &Board, mv: Move) -> MoveLegality {
    match validate_move(board, mv) {
        Ok(()) => MoveLegality::Legal,
        Err(err) => MoveLegality::Illegal(err),
    }
}

impl Board {
    /// 在合法性校验通过后应用走法。
    pub fn apply_move_if_legal(&mut self, mv: Move) -> Result<crate::types::MoveOutcome, LegalMoveError> {
        validate_move(self, mv)?;
        self.apply_move_unchecked(mv).ok_or(LegalMoveError::NoPieceAtFrom)
    }
}

fn validate_move(board: &Board, mv: Move) -> Result<(), LegalMoveError> {
    if !mv.from.is_in_bounds() || !mv.to.is_in_bounds() {
        return Err(LegalMoveError::OutOfBounds);
    }

    let moving_piece = board.get(mv.from).ok_or(LegalMoveError::NoPieceAtFrom)?;

    if moving_piece.side != board.side_to_move {
        return Err(LegalMoveError::WrongSideToMove {
            expected: board.side_to_move,
            found: moving_piece.side,
        });
    }

    if let Some(target_piece) = board.get(mv.to) {
        if target_piece.side == moving_piece.side {
            return Err(LegalMoveError::FriendlyFire);
        }
    }

    if !piece_rule_allows(board, moving_piece, mv) {
        return Err(LegalMoveError::InvalidPieceRule(moving_piece.kind));
    }

    // v1.0 基础约束：不允许将帅照面。
    if kings_face_after_move(board, mv) {
        return Err(LegalMoveError::KingsFaceEachOther);
    }

    Ok(())
}

fn piece_rule_allows(board: &Board, piece: Piece, mv: Move) -> bool {
    match piece.kind {
        PieceKind::King => king_rule(piece.side, mv),
        PieceKind::Advisor => advisor_rule(piece.side, mv),
        PieceKind::Elephant => elephant_rule(board, piece.side, mv),
        PieceKind::Horse => horse_rule(board, mv),
        PieceKind::Rook => rook_rule(board, mv),
        PieceKind::Cannon => cannon_rule(board, mv),
        PieceKind::Pawn => pawn_rule(piece.side, mv),
    }
}

fn king_rule(side: Side, mv: Move) -> bool {
    let df = abs_diff(mv.from.file, mv.to.file);
    let dr = abs_diff(mv.from.rank, mv.to.rank);

    let is_one_step_ortho = (df == 1 && dr == 0) || (df == 0 && dr == 1);
    is_one_step_ortho && in_palace(side, mv.to)
}

fn advisor_rule(side: Side, mv: Move) -> bool {
    let df = abs_diff(mv.from.file, mv.to.file);
    let dr = abs_diff(mv.from.rank, mv.to.rank);
    df == 1 && dr == 1 && in_palace(side, mv.to)
}

fn elephant_rule(board: &Board, side: Side, mv: Move) -> bool {
    let df = abs_diff(mv.from.file, mv.to.file);
    let dr = abs_diff(mv.from.rank, mv.to.rank);
    if !(df == 2 && dr == 2) {
        return false;
    }

    // 不能过河。
    if crosses_river(side, mv.to) {
        return false;
    }

    // 象眼不能被堵。
    let eye_file = mid_u8(mv.from.file, mv.to.file);
    let eye_rank = mid_u8(mv.from.rank, mv.to.rank);
    let Some(eye) = Pos::new(eye_file, eye_rank) else {
        return false;
    };
    board.get(eye).is_none()
}

fn horse_rule(board: &Board, mv: Move) -> bool {
    let df = mv.to.file as i16 - mv.from.file as i16;
    let dr = mv.to.rank as i16 - mv.from.rank as i16;

    let adf = df.unsigned_abs();
    let adr = dr.unsigned_abs();

    let (leg_file, leg_rank) = match (adf, adr) {
        (2, 1) => (mv.from.file as i16 + df.signum(), mv.from.rank as i16),
        (1, 2) => (mv.from.file as i16, mv.from.rank as i16 + dr.signum()),
        _ => return false,
    };

    if !in_bounds_i16(leg_file, leg_rank) {
        return false;
    }
    let leg = Pos {
        file: leg_file as u8,
        rank: leg_rank as u8,
    };
    board.get(leg).is_none()
}

fn rook_rule(board: &Board, mv: Move) -> bool {
    let Some(count) = count_between_straight(board, mv.from, mv.to) else {
        return false;
    };
    count == 0
}

fn cannon_rule(board: &Board, mv: Move) -> bool {
    let Some(count) = count_between_straight(board, mv.from, mv.to) else {
        return false;
    };

    match board.get(mv.to) {
        None => count == 0,
        Some(_) => count == 1,
    }
}

fn pawn_rule(side: Side, mv: Move) -> bool {
    let df = mv.to.file as i16 - mv.from.file as i16;
    let dr = mv.to.rank as i16 - mv.from.rank as i16;

    // 兵/卒不能后退。
    if (side == Side::Red && dr < 0) || (side == Side::Black && dr > 0) {
        return false;
    }

    let forward_step = match side {
        Side::Red => 1,
        Side::Black => -1,
    };

    // 直进一格。
    if df == 0 && dr == forward_step {
        return true;
    }

    // 过河后可以平移一格。
    if has_crossed_river(side, mv.from) && dr == 0 && df.unsigned_abs() == 1 {
        return true;
    }

    false
}

fn kings_face_after_move(board: &Board, mv: Move) -> bool {
    let Some(next) = board.clone_with_move_unchecked(mv) else {
        return false;
    };
    kings_face_each_other(&next)
}

fn kings_face_each_other(board: &Board) -> bool {
    let Some(red_king) = board.find_king(Side::Red) else {
        return false;
    };
    let Some(black_king) = board.find_king(Side::Black) else {
        return false;
    };

    if red_king.file != black_king.file {
        return false;
    }

    let file = red_king.file;
    let min_rank = red_king.rank.min(black_king.rank);
    let max_rank = red_king.rank.max(black_king.rank);

    for rank in (min_rank + 1)..max_rank {
        let Some(pos) = Pos::new(file, rank) else {
            return false;
        };
        if board.get(pos).is_some() {
            return false;
        }
    }

    true
}

fn in_palace(side: Side, pos: Pos) -> bool {
    let file_ok = (3..=5).contains(&(pos.file as usize));
    let rank_ok = match side {
        Side::Red => (0..=2).contains(&(pos.rank as usize)),
        Side::Black => (7..=9).contains(&(pos.rank as usize)),
    };
    file_ok && rank_ok
}

fn crosses_river(side: Side, pos: Pos) -> bool {
    match side {
        Side::Red => pos.rank >= 5,
        Side::Black => pos.rank <= 4,
    }
}

fn has_crossed_river(side: Side, pos: Pos) -> bool {
    match side {
        Side::Red => pos.rank >= 5,
        Side::Black => pos.rank <= 4,
    }
}

fn count_between_straight(board: &Board, from: Pos, to: Pos) -> Option<usize> {
    if from.file == to.file {
        let file = from.file;
        let start = from.rank.min(to.rank) + 1;
        let end = from.rank.max(to.rank);
        let mut count = 0usize;
        for rank in start..end {
            let pos = Pos::new(file, rank)?;
            if board.get(pos).is_some() {
                count += 1;
            }
        }
        return Some(count);
    }

    if from.rank == to.rank {
        let rank = from.rank;
        let start = from.file.min(to.file) + 1;
        let end = from.file.max(to.file);
        let mut count = 0usize;
        for file in start..end {
            let pos = Pos::new(file, rank)?;
            if board.get(pos).is_some() {
                count += 1;
            }
        }
        return Some(count);
    }

    None
}

fn abs_diff(a: u8, b: u8) -> u8 {
    a.max(b) - a.min(b)
}

fn mid_u8(a: u8, b: u8) -> u8 {
    ((a as u16 + b as u16) / 2) as u8
}

fn in_bounds_i16(file: i16, rank: i16) -> bool {
    (0..BOARD_COLS as i16).contains(&file) && (0..BOARD_ROWS as i16).contains(&rank)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Piece, PieceKind};

    fn must_pos(s: &str) -> Pos {
        Pos::from_iccs_square(s).expect("合法坐标")
    }

    fn piece(side: Side, kind: PieceKind) -> Piece {
        Piece { side, kind }
    }

    /// 基础棋盘：双方将帅 + 一个阻挡子，避免天然照面。
    fn base_board(side_to_move: Side) -> Board {
        let mut board = Board::empty(side_to_move);
        board.set(must_pos("e0"), Some(piece(Side::Red, PieceKind::King)));
        board.set(must_pos("e9"), Some(piece(Side::Black, PieceKind::King)));
        // 使用高位阻挡子避免将帅照面，同时尽量不干扰常见测试走法。
        board.set(must_pos("e7"), Some(piece(Side::Black, PieceKind::Pawn)));
        board
    }

    fn assert_legal(board: &Board, mv: &str) {
        let mv = Move::from_iccs(mv).expect("合法 ICCS 字符串");
        match check_move_legality(board, mv) {
            MoveLegality::Legal => {}
            MoveLegality::Illegal(err) => {
                panic!("期望合法，但被判定为非法: {mv} => {err}");
            }
        }
    }

    fn assert_illegal(board: &Board, mv: &str) {
        let mv = Move::from_iccs(mv).expect("合法 ICCS 字符串");
        if matches!(check_move_legality(board, mv), MoveLegality::Legal) {
            panic!("期望非法，但被判定为合法: {mv}");
        }
    }

    #[test]
    fn king_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("e1"), Some(piece(Side::Red, PieceKind::King)));
        board.set(must_pos("e0"), None);

        // 合法（5）
        for mv in ["e1e0", "e1e2", "e1d1", "e1f1", "e1e0"] {
            assert_legal(&board, mv);
        }

        // 非法（>=5）
        for mv in [
            "e1e3", // 走两步
            "e1c1", // 出九宫
            "e1e4", // 走太远
            "e1d2", // 斜走
            "e1f2", // 斜走
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn advisor_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("e1"), Some(piece(Side::Red, PieceKind::Advisor)));

        for mv in ["e1d2", "e1f2", "e1d0", "e1f0", "e1d2"] {
            assert_legal(&board, mv);
        }

        for mv in [
            "e1e2", // 直走
            "e1g3", // 出九宫
            "e1c3", // 出九宫
            "e1e0", // 直走
            "e1d1", // 平移
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn elephant_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("c2"), Some(piece(Side::Red, PieceKind::Elephant)));

        // 合法（含不过河与象眼不堵）
        for mv in ["c2a4", "c2e4", "c2a0", "c2a4", "c2e4"] {
            assert_legal(&board, mv);
        }

        // 象眼被堵
        board.set(must_pos("d3"), Some(piece(Side::Red, PieceKind::Pawn)));
        assert_illegal(&board, "c2e4");
        board.set(must_pos("d3"), None);

        // 非法（>=5）
        for mv in [
            "c2e6", // 过河
            "c2b3", // 走一步
            "c2c4", // 直走
            "c2g6", // 过河且太远
            "c2a6", // 过河
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn horse_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("e4"), Some(piece(Side::Red, PieceKind::Horse)));

        // 合法（5）
        for mv in ["e4c5", "e4c3", "e4d6", "e4f6", "e4g5"] {
            assert_legal(&board, mv);
        }

        // 马腿被堵（e5 已有阻挡子）
        assert_illegal(&board, "e4e6");

        // 额外设置马腿堵塞
        board.set(must_pos("f4"), Some(piece(Side::Red, PieceKind::Pawn)));
        assert_illegal(&board, "e4g5");
        board.set(must_pos("f4"), None);

        for mv in [
            "e4e5", // 直走
            "e4e2", // 直走
            "e4g4", // 平移
            "e4h4", // 平移
            "e4d4", // 平移
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn rook_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("a0"), Some(piece(Side::Red, PieceKind::Rook)));

        // 合法（5）
        for mv in ["a0a1", "a0a5", "a0a9", "a0b0", "a0d0"] {
            assert_legal(&board, mv);
        }

        // 路径阻挡
        board.set(must_pos("a3"), Some(piece(Side::Red, PieceKind::Pawn)));
        assert_illegal(&board, "a0a5");
        board.set(must_pos("a3"), None);

        for mv in [
            "a0b1", // 斜走
            "a0c2", // 斜走
            "a0b2", // 斜走
            "a0c1", // 斜走
            "a0d3", // 斜走
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn cannon_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("b2"), Some(piece(Side::Red, PieceKind::Cannon)));
        board.set(must_pos("b6"), Some(piece(Side::Black, PieceKind::Pawn)));

        // 不吃子时路径必须无子
        assert_legal(&board, "b2b5");

        // 吃子时必须隔一个子
        board.set(must_pos("b4"), Some(piece(Side::Red, PieceKind::Pawn)));
        assert_legal(&board, "b2b6");

        // 隔子数量不对 => 非法
        board.set(must_pos("b3"), Some(piece(Side::Red, PieceKind::Pawn)));
        assert_illegal(&board, "b2b6");
        board.set(must_pos("b3"), None);
        board.set(must_pos("b4"), None);

        for mv in [
            "b2c3", // 斜走
            "b2d4", // 斜走
            "b2a3", // 斜走
            "b2c1", // 斜走
            "b2e5", // 斜走
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn pawn_rules_examples() {
        let mut board = base_board(Side::Red);
        board.set(must_pos("e4"), Some(piece(Side::Red, PieceKind::Pawn)));

        // 未过河只能前进
        assert_legal(&board, "e4e5");
        assert_illegal(&board, "e4d4");
        assert_illegal(&board, "e4e3");

        // 过河后可平移
        board.set(must_pos("e5"), Some(piece(Side::Red, PieceKind::Pawn)));
        board.set(must_pos("e4"), None);
        assert_legal(&board, "e5d5");
        assert_legal(&board, "e5f5");

        for mv in [
            "e5e4", // 后退
            "e5e3", // 后退
            "e5d4", // 斜走
            "e5f4", // 斜走
            "e5c5", // 横走两格
        ] {
            assert_illegal(&board, mv);
        }
    }

    #[test]
    fn kings_face_each_other_is_rejected() {
        let mut board = Board::empty(Side::Red);
        board.set(must_pos("e0"), Some(piece(Side::Red, PieceKind::King)));
        board.set(must_pos("e9"), Some(piece(Side::Black, PieceKind::King)));
        // 用一枚棋子阻挡照面。
        board.set(must_pos("e1"), Some(piece(Side::Red, PieceKind::Rook)));

        // 这步会移走阻挡子并保持其他规则合法，从而导致照面，应判定非法。
        assert_illegal(&board, "e1f1");
    }

    #[test]
    fn wrong_side_to_move_is_rejected() {
        let mut board = base_board(Side::Black);
        board.set(must_pos("a0"), Some(piece(Side::Red, PieceKind::Rook)));
        assert_illegal(&board, "a0a1");
    }
}
