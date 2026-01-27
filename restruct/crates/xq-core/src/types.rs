use core::fmt;

/// 中国象棋棋盘行数。
pub const BOARD_ROWS: usize = 10;
/// 中国象棋棋盘列数。
pub const BOARD_COLS: usize = 9;

/// 中国象棋标准开局 FEN（以红方先行 `w`）。
pub const STARTPOS_FEN: &str =
    "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w";

/// 行棋方（Side to move）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Side {
    /// 红方（通常用 `w` 表示先行方）。
    #[default]
    Red,
    /// 黑方。
    Black,
}

impl Side {
    /// 返回对手一方。
    #[must_use]
    pub const fn opposite(self) -> Self {
        match self {
            Self::Red => Self::Black,
            Self::Black => Self::Red,
        }
    }

    /// 将行棋方转为 FEN 使用的字符（`w` / `b`）。
    #[must_use]
    pub const fn to_fen_char(self) -> char {
        match self {
            Self::Red => 'w',
            Self::Black => 'b',
        }
    }

    /// 从 FEN 行棋方字符解析（支持 `w`/`b`，也容忍 `r`/`red`/`black` 的首字母）。
    #[must_use]
    pub fn from_fen_char(value: char) -> Option<Self> {
        match value {
            'w' | 'W' | 'r' | 'R' => Some(Self::Red),
            'b' | 'B' => Some(Self::Black),
            _ => None,
        }
    }
}

/// 棋子类型（不区分红黑）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PieceKind {
    King,
    Advisor,
    Elephant,
    Horse,
    Rook,
    Cannon,
    Pawn,
}

impl PieceKind {
    /// 返回红方棋子的 FEN 基础字符（会在 `Piece` 中根据阵营决定大小写）。
    #[must_use]
    pub const fn fen_base(self) -> char {
        match self {
            Self::King => 'K',
            Self::Advisor => 'A',
            Self::Elephant => 'B',
            Self::Horse => 'N',
            Self::Rook => 'R',
            Self::Cannon => 'C',
            Self::Pawn => 'P',
        }
    }
}

/// 单个棋子（包含阵营与类型）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub side: Side,
    pub kind: PieceKind,
}

impl Piece {
    /// 生成 FEN 字符：红方大写，黑方小写。
    #[must_use]
    pub const fn to_fen_char(self) -> char {
        let base = self.kind.fen_base();
        match self.side {
            Side::Red => base,
            Side::Black => base.to_ascii_lowercase(),
        }
    }

    /// 从 FEN 字符解析棋子。
    #[must_use]
    pub fn from_fen_char(value: char) -> Option<Self> {
        let side = if value.is_ascii_uppercase() {
            Side::Red
        } else if value.is_ascii_lowercase() {
            Side::Black
        } else {
            return None;
        };

        let kind = match value.to_ascii_uppercase() {
            'K' => PieceKind::King,
            'A' => PieceKind::Advisor,
            'B' => PieceKind::Elephant,
            'N' => PieceKind::Horse,
            'R' => PieceKind::Rook,
            'C' => PieceKind::Cannon,
            'P' => PieceKind::Pawn,
            _ => return None,
        };

        Some(Self { side, kind })
    }
}

/// 棋盘格子：要么为空，要么有一枚棋子。
pub type BoardCell = Option<Piece>;

/// ICCS 坐标（`a0`..`i9`），以红方视角为基准：
/// - `file`: 0..=8 对应 `a`..`i`
/// - `rank`: 0..=9 对应 `0`..`9`（`0` 为红方底线）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pos {
    pub file: u8,
    pub rank: u8,
}

impl Pos {
    /// 创建一个位置（不合法时返回 `None`）。
    #[must_use]
    pub const fn new(file: u8, rank: u8) -> Option<Self> {
        if file < BOARD_COLS as u8 && rank < BOARD_ROWS as u8 {
            Some(Self { file, rank })
        } else {
            None
        }
    }

    /// 是否在棋盘范围内。
    #[must_use]
    pub const fn is_in_bounds(self) -> bool {
        self.file < BOARD_COLS as u8 && self.rank < BOARD_ROWS as u8
    }

    /// 从 ICCS 方格字符串解析（如 `a0`）。
    pub fn from_iccs_square(value: &str) -> Result<Self, PosParseError> {
        let bytes = value.as_bytes();
        if bytes.len() != 2 {
            return Err(PosParseError::InvalidLength);
        }

        let file = bytes[0];
        let rank = bytes[1];

        if !(b'a'..=b'i').contains(&file) {
            return Err(PosParseError::InvalidFile(file as char));
        }
        if !(b'0'..=b'9').contains(&rank) {
            return Err(PosParseError::InvalidRank(rank as char));
        }

        let file_idx = file - b'a';
        let rank_idx = rank - b'0';

        Self::new(file_idx, rank_idx).ok_or(PosParseError::OutOfBounds)
    }

    /// 转为 ICCS 方格字符串（如 `h2`）。
    #[must_use]
    pub fn to_iccs_square(self) -> String {
        let file = (b'a' + self.file) as char;
        let rank = (b'0' + self.rank) as char;
        let mut out = String::with_capacity(2);
        out.push(file);
        out.push(rank);
        out
    }

    /// 转为内部数组索引（row, col）。
    ///
    /// 内部棋盘以“上方为高位 rank 9”的顺序存储：
    /// - `row = 9 - rank`
    /// - `col = file`
    #[must_use]
    pub const fn to_index(self) -> (usize, usize) {
        let row = (BOARD_ROWS as u8 - 1 - self.rank) as usize;
        let col = self.file as usize;
        (row, col)
    }

    /// 从内部数组索引（row, col）恢复为 ICCS 坐标。
    #[must_use]
    pub const fn from_index(row: usize, col: usize) -> Option<Self> {
        if row >= BOARD_ROWS || col >= BOARD_COLS {
            return None;
        }
        let rank = (BOARD_ROWS - 1 - row) as u8;
        let file = col as u8;
        Self::new(file, rank)
    }
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iccs_square())
    }
}

/// 位置解析错误。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosParseError {
    InvalidLength,
    InvalidFile(char),
    InvalidRank(char),
    OutOfBounds,
}

impl fmt::Display for PosParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => f.write_str("ICCS 方格长度必须为 2"),
            Self::InvalidFile(ch) => write!(f, "非法 file 字符: {ch}"),
            Self::InvalidRank(ch) => write!(f, "非法 rank 字符: {ch}"),
            Self::OutOfBounds => f.write_str("ICCS 坐标超出棋盘范围"),
        }
    }
}

impl std::error::Error for PosParseError {}

/// ICCS 走法（from -> to）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Move {
    pub from: Pos,
    pub to: Pos,
}

impl Move {
    /// 创建走法。
    #[must_use]
    pub const fn new(from: Pos, to: Pos) -> Self {
        Self { from, to }
    }

    /// 从 ICCS 走法字符串解析（如 `h2e2`）。
    pub fn from_iccs(value: &str) -> Result<Self, MoveParseError> {
        let bytes = value.as_bytes();
        if bytes.len() != 4 {
            return Err(MoveParseError::InvalidLength);
        }

        let from = Pos::from_iccs_square(&value[0..2]).map_err(MoveParseError::From)?;
        let to = Pos::from_iccs_square(&value[2..4]).map_err(MoveParseError::To)?;
        Ok(Self { from, to })
    }

    /// 转为 ICCS 走法字符串。
    #[must_use]
    pub fn to_iccs(self) -> String {
        let mut out = String::with_capacity(4);
        out.push_str(&self.from.to_iccs_square());
        out.push_str(&self.to.to_iccs_square());
        out
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iccs())
    }
}

/// 走法解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveParseError {
    InvalidLength,
    From(PosParseError),
    To(PosParseError),
}

impl fmt::Display for MoveParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => f.write_str("ICCS 走法长度必须为 4"),
            Self::From(err) => write!(f, "起点坐标非法: {err}"),
            Self::To(err) => write!(f, "终点坐标非法: {err}"),
        }
    }
}

impl std::error::Error for MoveParseError {}

/// 棋盘结构：10×9 的格子 + 行棋方。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub grid: [[BoardCell; BOARD_COLS]; BOARD_ROWS],
    pub side_to_move: Side,
}

impl Default for Board {
    fn default() -> Self {
        // 默认给出标准开局，便于大多数调用方直接使用。
        Self::startpos()
    }
}

impl Board {
    /// 创建一个空棋盘。
    #[must_use]
    pub fn empty(side_to_move: Side) -> Self {
        Self {
            grid: [[None; BOARD_COLS]; BOARD_ROWS],
            side_to_move,
        }
    }

    /// 标准开局。
    #[must_use]
    pub fn startpos() -> Self {
        // STARTPOS_FEN 在本 crate 内是常量，解析失败应视为程序错误。
        match crate::fen::parse_fen(STARTPOS_FEN, crate::fen::FenParseOptions::strict()) {
            Ok(board) => board,
            Err(err) => panic!("STARTPOS_FEN 解析失败: {err}"),
        }
    }

    /// 获取某个坐标的格子内容。
    #[must_use]
    pub fn get(&self, pos: Pos) -> BoardCell {
        let (row, col) = pos.to_index();
        self.grid[row][col]
    }

    /// 设置某个坐标的格子内容。
    pub fn set(&mut self, pos: Pos, cell: BoardCell) {
        let (row, col) = pos.to_index();
        self.grid[row][col] = cell;
    }

    /// 统计棋子数量。
    #[must_use]
    pub fn piece_count(&self) -> usize {
        self.grid
            .iter()
            .flat_map(|row| row.iter())
            .filter(|cell| cell.is_some())
            .count()
    }

    /// 查找某一方将/帅的位置。
    #[must_use]
    pub fn find_king(&self, side: Side) -> Option<Pos> {
        for row in 0..BOARD_ROWS {
            for col in 0..BOARD_COLS {
                if let Some(piece) = self.grid[row][col] {
                    if piece.side == side && piece.kind == PieceKind::King {
                        if let Some(pos) = Pos::from_index(row, col) {
                            return Some(pos);
                        }
                    }
                }
            }
        }
        None
    }

    /// 在不做合法性校验的情况下应用一步走法。
    pub fn apply_move_unchecked(&mut self, mv: Move) -> Option<MoveOutcome> {
        let moving_piece = self.get(mv.from)?;
        let captured = self.get(mv.to);

        self.set(mv.to, Some(moving_piece));
        self.set(mv.from, None);
        self.side_to_move = self.side_to_move.opposite();

        Some(MoveOutcome {
            mv,
            moved: moving_piece,
            captured,
            next_side_to_move: self.side_to_move,
        })
    }

    /// 复制棋盘并应用一步走法（不校验合法性）。
    #[must_use]
    pub fn clone_with_move_unchecked(&self, mv: Move) -> Option<Self> {
        let mut next = self.clone();
        next.apply_move_unchecked(mv)?;
        Some(next)
    }
}

/// 一步走子后的结果快照（用于棋谱记录）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveOutcome {
    pub mv: Move,
    pub moved: Piece,
    pub captured: BoardCell,
    pub next_side_to_move: Side,
}

/// 对局记录（最小可用）：当前局面 + 走子历史。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRecord {
    pub board: Board,
    pub history: Vec<MoveOutcome>,
}

impl GameRecord {
    /// 从一个棋盘创建对局记录。
    #[must_use]
    pub fn new(board: Board) -> Self {
        Self {
            board,
            history: Vec::new(),
        }
    }

    /// 使用标准开局创建对局记录。
    #[must_use]
    pub fn startpos() -> Self {
        Self::new(Board::startpos())
    }

    /// 直接应用一步走法（不校验合法性），并记录历史。
    pub fn apply_move_unchecked(&mut self, mv: Move) -> Option<MoveOutcome> {
        let outcome = self.board.apply_move_unchecked(mv)?;
        self.history.push(outcome);
        Some(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_has_32_pieces() {
        let board = Board::startpos();
        assert_eq!(board.piece_count(), 32);
        assert_eq!(board.side_to_move, Side::Red);
    }

    #[test]
    fn apply_move_changes_board_state() {
        let mut board = Board::startpos();
        let mv = Move::from_iccs("h2e2").expect("合法 ICCS");

        let from_piece = board.get(mv.from);
        assert!(from_piece.is_some(), "起点必须有棋子");

        let outcome = board.apply_move_unchecked(mv).expect("应能落子");
        assert_eq!(outcome.mv, mv);
        assert_eq!(board.get(mv.from), None);
        assert_eq!(board.get(mv.to), from_piece);
        assert_eq!(board.side_to_move, Side::Black);
    }
}
