use xq_core::{diff_boards, BoardDiffKind};

use crate::postprocess::BoardObservation;

/// 稳定性过滤参数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StabilitySettings {
    /// 连续 OneChanged 的最大容忍次数，超过后触发 reset。
    pub one_changed_limit: usize,
}

impl Default for StabilitySettings {
    fn default() -> Self {
        Self {
            one_changed_limit: 3,
        }
    }
}

/// 被拒绝或重置的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    OneChanged { count: usize, limit: usize },
    InvalidDiff { kind: BoardDiffKind },
}

/// 稳定性过滤的决策结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilityDecision {
    Accepted(BoardObservation),
    Unchanged(BoardObservation),
    Rejected(RejectReason),
    Reset(RejectReason),
}

/// 识别稳定性过滤器：候选事件必须先过这里，才能进入上层状态机。
#[derive(Debug, Clone)]
pub struct StabilityFilter {
    last_stable: Option<BoardObservation>,
    one_changed_count: usize,
    settings: StabilitySettings,
}

impl StabilityFilter {
    #[must_use]
    pub fn new(settings: StabilitySettings) -> Self {
        Self {
            last_stable: None,
            one_changed_count: 0,
            settings,
        }
    }

    #[must_use]
    pub fn last_stable(&self) -> Option<&BoardObservation> {
        self.last_stable.as_ref()
    }

    /// 输入一次候选观测并给出过滤结果。
    pub fn process(&mut self, observation: BoardObservation) -> StabilityDecision {
        let Some(last) = self.last_stable.as_ref() else {
            self.one_changed_count = 0;
            self.last_stable = Some(observation.clone());
            return StabilityDecision::Accepted(observation);
        };

        if observation.board == last.board {
            self.one_changed_count = 0;
            self.last_stable = Some(observation.clone());
            return StabilityDecision::Unchanged(observation);
        }

        let diff = diff_boards(&last.board, &observation.board);
        match diff.kind {
            BoardDiffKind::NoChange => {
                self.one_changed_count = 0;
                self.last_stable = Some(observation.clone());
                StabilityDecision::Unchanged(observation)
            }
            BoardDiffKind::MoveCandidate => {
                self.one_changed_count = 0;
                self.last_stable = Some(observation.clone());
                StabilityDecision::Accepted(observation)
            }
            BoardDiffKind::OneChanged => {
                self.one_changed_count += 1;
                let reason = RejectReason::OneChanged {
                    count: self.one_changed_count,
                    limit: self.settings.one_changed_limit,
                };
                if self.one_changed_count >= self.settings.one_changed_limit {
                    self.one_changed_count = 0;
                    self.last_stable = None;
                    StabilityDecision::Reset(reason)
                } else {
                    StabilityDecision::Rejected(reason)
                }
            }
            other => StabilityDecision::Rejected(RejectReason::InvalidDiff { kind: other }),
        }
    }

    /// 外部可在必要时主动 reset（例如窗口尺寸变化）。
    pub fn reset(&mut self) {
        self.last_stable = None;
        self.one_changed_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use xq_core::{Board, Move, Piece, PieceKind, Pos, Side};

    use super::*;
    use crate::postprocess::{BoardObservation, Camp};

    fn pos(file: u8, rank: u8) -> Pos {
        Pos::new(file, rank).unwrap()
    }

    fn base_board() -> Board {
        let mut board = Board::empty(Side::Red);
        board.set(pos(4, 0), Some(Piece { side: Side::Red, kind: PieceKind::King }));
        board.set(pos(4, 9), Some(Piece { side: Side::Black, kind: PieceKind::King }));
        board.set(pos(0, 0), Some(Piece { side: Side::Red, kind: PieceKind::Rook }));
        board
    }

    fn obs_from_board(board: Board) -> BoardObservation {
        BoardObservation {
            camp: Camp::Unknown,
            flipped: false,
            raw_grid: [[None; xq_core::BOARD_COLS]; xq_core::BOARD_ROWS],
            board,
        }
    }

    #[test]
    fn first_observation_is_accepted() {
        let mut filter = StabilityFilter::new(StabilitySettings { one_changed_limit: 2 });
        let decision = filter.process(obs_from_board(base_board()));
        assert!(matches!(decision, StabilityDecision::Accepted(_)));
    }

    #[test]
    fn move_candidate_is_accepted() {
        let mut filter = StabilityFilter::new(StabilitySettings::default());
        let first = obs_from_board(base_board());
        assert!(matches!(filter.process(first), StabilityDecision::Accepted(_)));

        let mut next_board = base_board();
        let mv = Move::new(pos(0, 0), pos(0, 1));
        next_board.apply_move_unchecked(mv).expect("走子应成功");
        next_board.side_to_move = Side::Red;

        let decision = filter.process(obs_from_board(next_board));
        assert!(matches!(decision, StabilityDecision::Accepted(_)));
    }

    #[test]
    fn one_changed_triggers_reset_after_limit() {
        let mut filter = StabilityFilter::new(StabilitySettings { one_changed_limit: 2 });
        let base = base_board();
        let _ = filter.process(obs_from_board(base.clone()));

        let mut glitch = base.clone();
        glitch.set(pos(0, 0), Some(Piece { side: Side::Red, kind: PieceKind::Horse }));
        glitch.side_to_move = Side::Red;

        let first = filter.process(obs_from_board(glitch.clone()));
        assert!(matches!(first, StabilityDecision::Rejected(RejectReason::OneChanged { .. })));

        let second = filter.process(obs_from_board(glitch));
        assert!(matches!(second, StabilityDecision::Reset(RejectReason::OneChanged { .. })));
        assert!(filter.last_stable().is_none());
    }
}
