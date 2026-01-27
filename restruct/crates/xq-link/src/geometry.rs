use xq_core::{map_pos_with_flip, Pos, BOARD_COLS, BOARD_ROWS};
use xq_vision::crop::CropRegion;

use crate::window::WindowPosition;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenPoint {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardGeometry {
    pub region: CropRegion,
    pub padding_cells: f32,
    pub board_x: f32,
    pub board_y: f32,
    pub board_w: f32,
    pub board_h: f32,
    pub cell_w: f32,
    pub cell_h: f32,
}

impl BoardGeometry {
    pub fn from_crop(region: CropRegion, padding_cells: f32) -> Self {
        let padding_cells = padding_cells.max(0.0);
        let pad_x_factor = 1.0 + padding_cells / BOARD_COLS as f32;
        let pad_y_factor = 1.0 + padding_cells / BOARD_ROWS as f32;

        let board_w = (region.width as f32 / pad_x_factor).max(1.0);
        let board_h = (region.height as f32 / pad_y_factor).max(1.0);

        let half_cell_x = board_w / BOARD_COLS as f32 / 2.0 * padding_cells;
        let half_cell_y = board_h / BOARD_ROWS as f32 / 2.0 * padding_cells;

        let board_x = region.x as f32 + half_cell_x;
        let board_y = region.y as f32 + half_cell_y;

        let cell_w = board_w / BOARD_COLS as f32;
        let cell_h = board_h / BOARD_ROWS as f32;

        Self {
            region,
            padding_cells,
            board_x,
            board_y,
            board_w,
            board_h,
            cell_w,
            cell_h,
        }
    }

    pub fn window_point_for_pos(&self, pos: Pos, flipped: bool) -> (f32, f32) {
        let mapped = map_pos_with_flip(pos, flipped);
        let (row, col) = mapped.to_index();
        let x = self.board_x + (col as f32 + 0.5) * self.cell_w;
        let y = self.board_y + (row as f32 + 0.5) * self.cell_h;
        (x, y)
    }

    pub fn screen_point_for_pos(
        &self,
        pos: Pos,
        flipped: bool,
        window: WindowPosition,
        apply_scale_factor: bool,
    ) -> ScreenPoint {
        let (x, y) = self.window_point_for_pos(pos, flipped);
        let scale = if apply_scale_factor {
            window.scale_factor.max(0.1)
        } else {
            1.0
        };
        let sx = window.x as f32 + x / scale;
        let sy = window.y as f32 + y / scale;
        ScreenPoint {
            x: sx.round() as i32,
            y: sy.round() as i32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn geometry_padding_adjusts_board_rect() {
        let region = CropRegion {
            x: 0,
            y: 0,
            width: 900,
            height: 1000,
        };
        let geom = BoardGeometry::from_crop(region, 1.0);
        assert!(geom.board_x > 0.0);
        assert!(geom.board_y > 0.0);
        assert!(geom.board_w < 900.0);
        assert!(geom.board_h < 1000.0);
    }

    #[test]
    fn geometry_maps_positions_with_flip() {
        let region = CropRegion {
            x: 0,
            y: 0,
            width: 900,
            height: 1000,
        };
        let geom = BoardGeometry::from_crop(region, 0.0);
        let pos = Pos::new(0, 0).expect("a0");

        let (x, y) = geom.window_point_for_pos(pos, false);
        assert!(approx_eq(x, 50.0));
        assert!(approx_eq(y, 950.0));

        let (xf, yf) = geom.window_point_for_pos(pos, true);
        assert!(approx_eq(xf, 850.0));
        assert!(approx_eq(yf, 50.0));
    }
}
