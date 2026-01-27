//! xq-core: 中国象棋正式版的领域核心（Step 2：规则与数据模型）。

mod diff;
mod fen;
mod flip;
mod legality;
mod types;

pub use diff::{diff_boards, BoardDiff, BoardDiffKind, DiffedMove};
pub use fen::{FenError, FenParseOptions};
pub use flip::{flip_move, flip_pos, map_pos_with_flip};
pub use legality::{check_move_legality, LegalMoveError, MoveLegality};
pub use types::{
    Board, BoardCell, GameRecord, Move, MoveOutcome, Piece, PieceKind, Pos, Side, BOARD_COLS,
    BOARD_ROWS, STARTPOS_FEN,
};

/// 返回 core crate 的版本标识，便于跨 crate 的最小连通性测试。
pub fn core_version() -> &'static str {
    "xq-core/0.2.0-step2"
}

#[cfg(test)]
mod tests {
    use super::core_version;

    #[test]
    fn core_version_is_stable() {
        assert_eq!(core_version(), "xq-core/0.2.0-step2");
    }
}
