use thiserror::Error;
use tracing::trace;

use xq_core::{flip_pos, Board, Piece, Pos, Side, BOARD_COLS, BOARD_ROWS};

use crate::detect::{Detection, IMAGE_HEIGHT, IMAGE_WIDTH};

const MODEL_CELL_W: f32 = IMAGE_WIDTH as f32 / BOARD_COLS as f32;
const MODEL_CELL_H: f32 = IMAGE_HEIGHT as f32 / BOARD_ROWS as f32;

/// 识别得到的阵营/视角信息。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Camp {
    Red,
    Black,
    Unknown,
}

impl Camp {
    #[must_use]
    pub fn is_black_bottom(self) -> bool {
        matches!(self, Self::Black)
    }
}

/// 识别阶段的结构化观测结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardObservation {
    pub camp: Camp,
    /// 是否对坐标做了 180° 翻转以回到红方底线视角。
    pub flipped: bool,
    pub board: Board,
    /// 原始网格（模型坐标系，row=0 为上方）。
    pub raw_grid: [[Option<char>; BOARD_COLS]; BOARD_ROWS],
}

/// 视觉识别错误。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VisionError {
    #[error("未检测到棋盘框(label=0)")]
    BoardNotFound,
    #[error("检测结果越界: row={row}, col={col}")]
    OutOfBounds { row: usize, col: usize },
}

/// 将 detections 映射到 10x9 网格，并推断 camp。
pub fn detections_to_grid(
    detections: &[Detection],
) -> Result<(Camp, [[Option<char>; BOARD_COLS]; BOARD_ROWS]), VisionError> {
    if detections.iter().all(|d| d.label != '0') {
        return Err(VisionError::BoardNotFound);
    }

    let mut camp = Camp::Unknown;
    let mut grid = [[None; BOARD_COLS]; BOARD_ROWS];

    for det in detections.iter().filter(|d| d.label != '0') {
        let (cx, cy) = det.center();
        let col = (cx / MODEL_CELL_W).floor() as usize;
        let row = (cy / MODEL_CELL_H).floor() as usize;
        trace!(target: "xq_vision", label = %det.label, row, col, "detection mapped to grid");

        if col >= BOARD_COLS || row >= BOARD_ROWS {
            return Err(VisionError::OutOfBounds { row, col });
        }

        grid[row][col] = Some(det.label);

        if matches!(camp, Camp::Unknown) && (3..=5).contains(&col) && row >= 7 {
            match det.label {
                'k' => camp = Camp::Black,
                'K' => camp = Camp::Red,
                _ => {}
            }
        }
    }

    Ok((camp, grid))
}

/// 将 detections 直接转换为 xq-core 的 Board 观测。
pub fn detections_to_observation(
    detections: &[Detection],
    side_to_move: Side,
) -> Result<BoardObservation, VisionError> {
    let (camp, grid) = detections_to_grid(detections)?;

    let mut board = Board::empty(side_to_move);
    let flipped = camp.is_black_bottom();

    for row in 0..BOARD_ROWS {
        for col in 0..BOARD_COLS {
            let Some(label) = grid[row][col] else {
                continue;
            };
            let Some(piece) = Piece::from_fen_char(label) else {
                continue;
            };
            let Some(pos) = Pos::from_index(row, col) else {
                return Err(VisionError::OutOfBounds { row, col });
            };
            let mapped = if flipped { flip_pos(pos) } else { pos };
            board.set(mapped, Some(piece));
        }
    }

    Ok(BoardObservation {
        camp,
        flipped,
        board,
        raw_grid: grid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::LABELS;

    fn det_from_center(cx: f32, cy: f32, label: char) -> Detection {
        let class_idx = LABELS.iter().position(|&c| c == label).unwrap();
        let w = MODEL_CELL_W * 0.8;
        let h = MODEL_CELL_H * 0.8;
        Detection {
            x0: cx - w / 2.0,
            x1: cx + w / 2.0,
            y0: cy - h / 2.0,
            y1: cy + h / 2.0,
            confidence: 0.99,
            label,
            class_idx,
            area: w * h,
        }
    }

    fn board_det() -> Detection {
        det_from_center(IMAGE_WIDTH as f32 / 2.0, IMAGE_HEIGHT as f32 / 2.0, '0')
    }

    fn cell_center(row: usize, col: usize) -> (f32, f32) {
        let cx = (col as f32 + 0.5) * MODEL_CELL_W;
        let cy = (row as f32 + 0.5) * MODEL_CELL_H;
        (cx, cy)
    }

    #[test]
    fn detections_require_board() {
        let (cx, cy) = cell_center(9, 4);
        let dets = vec![det_from_center(cx, cy, 'K')];
        let err = detections_to_grid(&dets).expect_err("没有棋盘框应报错");
        assert_eq!(err, VisionError::BoardNotFound);
    }

    #[test]
    fn red_bottom_maps_directly() {
        let (cx, cy) = cell_center(9, 4);
        let dets = vec![board_det(), det_from_center(cx, cy, 'K')];
        let obs = detections_to_observation(&dets, Side::Red).expect("应能生成观测");
        assert_eq!(obs.camp, Camp::Red);
        assert!(!obs.flipped);

        let king_pos = Pos::new(4, 0).unwrap();
        let king = obs.board.get(king_pos).expect("红帅应在 e0");
        assert_eq!(king.side, Side::Red);
    }

    #[test]
    fn black_bottom_is_flipped() {
        let (cx, cy) = cell_center(9, 4);
        let dets = vec![board_det(), det_from_center(cx, cy, 'k')];
        let obs = detections_to_observation(&dets, Side::Red).expect("应能生成观测");
        assert_eq!(obs.camp, Camp::Black);
        assert!(obs.flipped);

        let king_pos = Pos::new(4, 9).unwrap();
        let king = obs.board.get(king_pos).expect("黑将应翻转到 e9");
        assert_eq!(king.side, Side::Black);
    }
}
