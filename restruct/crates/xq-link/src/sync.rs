use anyhow::{anyhow, Result};
use xq_core::{
    check_move_legality, diff_boards, Board, BoardDiff, BoardDiffKind, DiffedMove, Move,
    MoveLegality,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPolicy {
    ExternalDriven,
    LocalDriven,
    Bidirectional,
}

impl SyncPolicy {
    pub const fn allows_external(self) -> bool {
        matches!(self, SyncPolicy::ExternalDriven | SyncPolicy::Bidirectional)
    }

    pub const fn allows_local(self) -> bool {
        matches!(self, SyncPolicy::LocalDriven | SyncPolicy::Bidirectional)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesyncReason {
    ExternalMismatch,
    InjectConfirmFailed,
    UnexpectedExternalChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentStatus {
    AwaitingExternal,
    Aligned,
    Aligning,
    Desynced(DesyncReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInjection {
    pub mv: Move,
    pub expected: Board,
    pub attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalUpdate {
    FirstSeen { aligned: bool },
    NoChange,
    CandidateMove {
        diff: BoardDiff,
        applied_to_local: bool,
    },
    Changed { diff: BoardDiff },
    PendingConfirmed { mv: Move },
    PendingRejected { diff: BoardDiff },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncConfig {
    pub policy: SyncPolicy,
    pub verify_legality: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            policy: SyncPolicy::Bidirectional,
            verify_legality: true,
        }
    }
}

pub struct SyncState {
    config: SyncConfig,
    status: AlignmentStatus,
    local: Board,
    external: Option<Board>,
    pending: Option<PendingInjection>,
}

impl SyncState {
    pub fn new(local: Board, config: SyncConfig) -> Self {
        Self {
            config,
            status: AlignmentStatus::AwaitingExternal,
            local,
            external: None,
            pending: None,
        }
    }

    pub fn status(&self) -> AlignmentStatus {
        self.status
    }

    pub fn local(&self) -> &Board {
        &self.local
    }

    pub fn external(&self) -> Option<&Board> {
        self.external.as_ref()
    }

    pub fn pending(&self) -> Option<&PendingInjection> {
        self.pending.as_ref()
    }

    pub fn set_verify_legality(&mut self, value: bool) {
        self.config.verify_legality = value;
    }

    pub fn set_local(&mut self, board: Board) {
        self.local = board;
        self.pending = None;
        self.status = if self.external.as_ref() == Some(&self.local) {
            AlignmentStatus::Aligned
        } else if self.external.is_none() {
            AlignmentStatus::AwaitingExternal
        } else {
            AlignmentStatus::Desynced(DesyncReason::ExternalMismatch)
        };
    }

    pub fn align_to_external(&mut self) -> Result<()> {
        let external = self.external.as_ref().ok_or_else(|| anyhow!("外部局面尚未就绪"))?;
        self.local = external.clone();
        self.pending = None;
        self.status = AlignmentStatus::Aligned;
        Ok(())
    }

    pub fn prepare_injection(&mut self, mv: Move) -> Result<PendingInjection> {
        if !self.config.policy.allows_local() {
            return Err(anyhow!("当前同步策略不允许本地驱动"));
        }
        if matches!(self.status, AlignmentStatus::Desynced(_)) {
            return Err(anyhow!("当前处于不同步状态"));
        }
        if self.pending.is_some() {
            return Err(anyhow!("已有待确认的注入"));
        }
        if self.config.verify_legality {
            match check_move_legality(&self.local, mv) {
                MoveLegality::Legal => {}
                MoveLegality::Illegal(err) => {
                    return Err(anyhow!("非法走法: {err}"));
                }
            }
        }
        let expected = self
            .local
            .clone_with_move_unchecked(mv)
            .ok_or_else(|| anyhow!("无法应用走法"))?;
        let pending = PendingInjection {
            mv,
            expected: expected.clone(),
            attempts: 0,
        };
        self.pending = Some(pending.clone());
        self.status = AlignmentStatus::Aligning;
        Ok(pending)
    }

    pub fn ingest_external(&mut self, board: Board) -> ExternalUpdate {
        if self.external.is_none() {
            let aligned = board == self.local;
            self.external = Some(board);
            self.status = if aligned {
                AlignmentStatus::Aligned
            } else {
                AlignmentStatus::Desynced(DesyncReason::ExternalMismatch)
            };
            return ExternalUpdate::FirstSeen { aligned };
        }

        if let Some(pending) = self.pending.take() {
            if board == pending.expected {
                self.local = pending.expected.clone();
                self.external = Some(board);
                self.status = AlignmentStatus::Aligned;
                return ExternalUpdate::PendingConfirmed { mv: pending.mv };
            }
            let prev = self.external.as_ref().expect("external exists");
            let diff = diff_boards(prev, &board);
            self.external = Some(board);
            self.status = AlignmentStatus::Desynced(DesyncReason::InjectConfirmFailed);
            return ExternalUpdate::PendingRejected { diff };
        }

        let prev = self.external.as_ref().expect("external exists").clone();
        let diff = diff_boards(&prev, &board);
        self.external = Some(board.clone());

        if diff.kind == BoardDiffKind::NoChange {
            return ExternalUpdate::NoChange;
        }

        if diff.kind == BoardDiffKind::MoveCandidate && diff.candidate.is_some() {
            let mut applied = false;
            if self.config.policy.allows_external()
                && matches!(self.status, AlignmentStatus::Aligned)
                && self.local == prev
            {
                if let Some(DiffedMove { mv, .. }) = diff.candidate {
                    if self.local.apply_move_unchecked(mv).is_some() {
                        applied = true;
                    }
                }
            }
            if self.local == board {
                self.status = AlignmentStatus::Aligned;
            } else {
                self.status = AlignmentStatus::Desynced(DesyncReason::ExternalMismatch);
            }
            return ExternalUpdate::CandidateMove {
                diff,
                applied_to_local: applied,
            };
        }

        self.status = AlignmentStatus::Desynced(DesyncReason::UnexpectedExternalChange);
        ExternalUpdate::Changed { diff }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xq_core::{Board, Move};

    fn must_move(value: &str) -> Move {
        Move::from_iccs(value).expect("valid iccs")
    }

    #[test]
    fn sync_first_external_aligned() {
        let board = Board::startpos();
        let mut sync = SyncState::new(board.clone(), SyncConfig::default());
        let update = sync.ingest_external(board.clone());
        assert_eq!(sync.status(), AlignmentStatus::Aligned);
        assert!(matches!(update, ExternalUpdate::FirstSeen { aligned: true }));
    }

    #[test]
    fn sync_external_move_applies_to_local() {
        let board = Board::startpos();
        let mut sync = SyncState::new(board.clone(), SyncConfig::default());
        sync.ingest_external(board.clone());

        let mv = must_move("h2e2");
        let mut external = board.clone();
        external.apply_move_unchecked(mv).expect("apply");

        let update = sync.ingest_external(external.clone());
        assert!(matches!(
            update,
            ExternalUpdate::CandidateMove {
                applied_to_local: true,
                ..
            }
        ));
        assert_eq!(sync.local(), &external);
        assert_eq!(sync.status(), AlignmentStatus::Aligned);
    }

    #[test]
    fn sync_pending_confirmed_updates_local() {
        let board = Board::startpos();
        let mut sync = SyncState::new(board.clone(), SyncConfig::default());
        sync.ingest_external(board.clone());

        let mv = must_move("h2e2");
        let pending = sync.prepare_injection(mv).expect("prepare");
        let update = sync.ingest_external(pending.expected.clone());
        assert!(matches!(update, ExternalUpdate::PendingConfirmed { .. }));
        assert_eq!(sync.local(), &pending.expected);
        assert_eq!(sync.status(), AlignmentStatus::Aligned);
    }

    #[test]
    fn sync_pending_rejected_desyncs() {
        let board = Board::startpos();
        let mut sync = SyncState::new(board.clone(), SyncConfig::default());
        sync.ingest_external(board.clone());

        let mv = must_move("h2e2");
        let _pending = sync.prepare_injection(mv).expect("prepare");
        let mut external = board.clone();
        external
            .apply_move_unchecked(must_move("a0a1"))
            .expect("apply");

        let update = sync.ingest_external(external);
        assert!(matches!(
            sync.status(),
            AlignmentStatus::Desynced(DesyncReason::InjectConfirmFailed)
        ));
        assert!(matches!(update, ExternalUpdate::PendingRejected { .. }));
    }
}
