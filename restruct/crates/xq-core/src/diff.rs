use crate::types::{BOARD_COLS, BOARD_ROWS, Board, BoardCell, Move, Piece, Pos};

/// board diff 的分类结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardDiffKind {
    /// 没有变化。
    NoChange,
    /// 推导出一步“候选走法”。
    MoveCandidate,
    /// 仅发现 1 处变化（典型识别抖动/漏检）。
    OneChanged,
    /// 发现多处变化。
    MultiChanged,
    /// 棋子数量异常减少。
    LessPieces,
    /// 棋子数量异常增加。
    MorePieces,
    /// 无法判定。
    Indeterminate,
}

/// 从两帧棋盘推导出的候选走法信息（不会直接落盘）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiffedMove {
    pub mv: Move,
    pub moved: Piece,
    pub captured: BoardCell,
}

/// board diff 结果（用于上层状态机确认/过滤）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardDiff {
    pub kind: BoardDiffKind,
    /// 新旧棋盘的棋子数量差：`new - old`。
    pub piece_count_delta: isize,
    /// 发生变化的格子（ICCS 坐标）。
    pub changed: Vec<Pos>,
    /// 若能推导出候选走法，则给出候选走法。
    pub candidate: Option<DiffedMove>,
}

impl BoardDiff {
    /// 是否为“可进一步确认的候选走法”。
    #[must_use]
    pub const fn is_move_candidate(&self) -> bool {
        matches!(self.kind, BoardDiffKind::MoveCandidate) && self.candidate.is_some()
    }
}

/// 计算两帧棋盘的差异。
#[must_use]
pub fn diff_boards(old: &Board, new: &Board) -> BoardDiff {
    let mut changed = Vec::new();
    let mut diffs: Vec<(Pos, BoardCell, BoardCell)> = Vec::new();

    let mut old_count = 0usize;
    let mut new_count = 0usize;

    for row in 0..BOARD_ROWS {
        for col in 0..BOARD_COLS {
            let old_cell = old.grid[row][col];
            let new_cell = new.grid[row][col];

            if old_cell.is_some() {
                old_count += 1;
            }
            if new_cell.is_some() {
                new_count += 1;
            }

            if old_cell == new_cell {
                continue;
            }
            if let Some(pos) = Pos::from_index(row, col) {
                changed.push(pos);
                diffs.push((pos, old_cell, new_cell));
            }
        }
    }

    let piece_count_delta = new_count as isize - old_count as isize;

    if diffs.is_empty() {
        return BoardDiff { kind: BoardDiffKind::NoChange, piece_count_delta, changed, candidate: None };
    }

    let candidate = infer_move_candidate(&diffs);

    let kind = classify_diff(diffs.len(), piece_count_delta, candidate.is_some());

    BoardDiff { kind, piece_count_delta, changed, candidate }
}

fn classify_diff(diff_len: usize, piece_count_delta: isize, has_candidate: bool) -> BoardDiffKind {
    // 优先识别明确的候选走法（允许吃子导致 -1）。
    if has_candidate && matches!(piece_count_delta, 0 | -1) && diff_len == 2 {
        return BoardDiffKind::MoveCandidate;
    }

    // 只要棋子数量出现异常（且不是合法候选走法），优先标记少子/多子。
    if piece_count_delta < 0 {
        return BoardDiffKind::LessPieces;
    }
    if piece_count_delta > 0 {
        return BoardDiffKind::MorePieces;
    }

    match diff_len {
        0 => BoardDiffKind::NoChange,
        1 => BoardDiffKind::OneChanged,
        2 => BoardDiffKind::Indeterminate,
        _ => BoardDiffKind::MultiChanged,
    }
}

fn infer_move_candidate(diffs: &[(Pos, BoardCell, BoardCell)]) -> Option<DiffedMove> {
    if diffs.len() != 2 {
        return None;
    }

    let mut from: Option<(Pos, Piece)> = None;
    let mut to: Option<(Pos, Piece, BoardCell)> = None;

    for (pos, old_cell, new_cell) in diffs.iter().copied() {
        match (old_cell, new_cell) {
            (Some(piece), None) => {
                from = Some((pos, piece));
            }
            (Some(old_piece), Some(new_piece)) => {
                // 两边都有棋子变化时，仍可能是“吃子 + 识别错误”。
                // 这里要求新位置的棋子必须与 from 的棋子同一枚，
                // 否则不当作候选走法。
                to = Some((pos, new_piece, Some(old_piece)));
            }
            (None, Some(piece)) => {
                to = Some((pos, piece, None));
            }
            (None, None) => {}
        }
    }

    let (from_pos, from_piece) = from?;
    let (to_pos, to_piece, captured) = to?;

    if from_piece != to_piece {
        return None;
    }

    Some(DiffedMove { mv: Move::new(from_pos, to_pos), moved: from_piece, captured })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Board, Move, Piece, PieceKind, Side};

    fn must_pos(s: &str) -> Pos {
        Pos::from_iccs_square(s).expect("合法坐标")
    }

    #[test]
    fn diff_detects_simple_move_candidate() {
        let old = Board::startpos();
        let mv = Move::from_iccs("h2e2").expect("合法走法字符串");

        let mut new = old.clone();
        new.apply_move_unchecked(mv).expect("可落子");

        let diff = diff_boards(&old, &new);
        assert!(diff.is_move_candidate());
        assert_eq!(diff.kind, BoardDiffKind::MoveCandidate);
        assert_eq!(diff.candidate.expect("候选走法").mv, mv);
        assert_eq!(diff.piece_count_delta, 0);
    }

    #[test]
    fn diff_detects_capture_move_candidate() {
        let mut old = Board::empty(Side::Red);

        let red_rook = Piece { side: Side::Red, kind: PieceKind::Rook };
        let black_pawn = Piece { side: Side::Black, kind: PieceKind::Pawn };

        old.set(must_pos("a0"), Some(red_rook));
        old.set(must_pos("a3"), Some(black_pawn));

        let mv = Move::from_iccs("a0a3").expect("ICCS");
        let mut new = old.clone();
        new.apply_move_unchecked(mv).expect("可落子");

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::MoveCandidate);
        assert_eq!(diff.piece_count_delta, -1);
        let candidate = diff.candidate.expect("候选走法");
        assert_eq!(candidate.mv, mv);
        assert_eq!(candidate.captured, Some(black_pawn));
    }

    #[test]
    fn diff_one_changed_is_classified() {
        let old = Board::startpos();
        let mut new = old.clone();

        new.set(must_pos("a0"), None);

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::LessPieces);
        assert_eq!(diff.changed.len(), 1);
        assert!(diff.candidate.is_none());

        // diff 仅返回候选事件，不会修改原棋盘。
        assert!(old.get(must_pos("a0")).is_some());
    }

    #[test]
    fn diff_more_pieces_is_classified() {
        let old = Board::empty(Side::Red);
        let mut new = old.clone();

        let red_pawn = Piece { side: Side::Red, kind: PieceKind::Pawn };
        new.set(must_pos("e4"), Some(red_pawn));
        new.set(must_pos("e5"), Some(red_pawn));

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::MorePieces);
        assert_eq!(diff.piece_count_delta, 2);
    }

    #[test]
    fn diff_less_pieces_is_classified() {
        let mut old = Board::empty(Side::Red);
        let mut new = old.clone();

        let red_pawn = Piece { side: Side::Red, kind: PieceKind::Pawn };
        old.set(must_pos("e4"), Some(red_pawn));
        old.set(must_pos("e5"), Some(red_pawn));
        new.set(must_pos("e4"), Some(red_pawn));
        new.set(must_pos("e5"), None);

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::LessPieces);
        assert_eq!(diff.piece_count_delta, -1);
    }

    #[test]
    fn diff_multi_changed_is_classified() {
        let mut old = Board::empty(Side::Red);
        let mut new = old.clone();

        let red_rook = Piece { side: Side::Red, kind: PieceKind::Rook };
        old.set(must_pos("a0"), Some(red_rook));
        old.set(must_pos("b0"), Some(red_rook));
        new.set(must_pos("a1"), Some(red_rook));
        new.set(must_pos("b1"), Some(red_rook));

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::MultiChanged);
        assert_eq!(diff.changed.len(), 4);
    }

    #[test]
    fn diff_indeterminate_when_two_changes_but_not_move() {
        let mut old = Board::empty(Side::Red);
        let mut new = old.clone();

        let red_rook = Piece { side: Side::Red, kind: PieceKind::Rook };
        let red_horse = Piece { side: Side::Red, kind: PieceKind::Horse };

        old.set(must_pos("a0"), Some(red_rook));
        old.set(must_pos("b0"), Some(red_horse));
        new.set(must_pos("a0"), Some(red_horse));
        new.set(must_pos("b0"), Some(red_rook));

        let diff = diff_boards(&old, &new);
        assert_eq!(diff.kind, BoardDiffKind::Indeterminate);
        assert!(diff.candidate.is_none());
    }

    #[test]
    fn diff_handles_no_change() {
        let board = Board::startpos();
        let diff = diff_boards(&board, &board);
        assert_eq!(diff.kind, BoardDiffKind::NoChange);
        assert_eq!(diff.changed.len(), 0);
    }
}
