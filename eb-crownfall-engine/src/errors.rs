use crate::{CrownfallBoardCell, CrownfallPlayerKind};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrownfallError {
    #[error("Player {0:?} tried to remove an nonexistent knight at {1:?}")]
    EmptyKnightRemoval(CrownfallPlayerKind, CrownfallBoardCell),
    #[error("Player {0:?} tried to remove an enemy knight at {1:?}")]
    EnemyKnightRemoval(CrownfallPlayerKind, CrownfallBoardCell),
    #[error("Player {0:?} tried to act out of turn")]
    NotYourTurn(CrownfallPlayerKind),
    #[error("Player {0:?} tried to act after the game had already ended")]
    GameOver(CrownfallPlayerKind),
    #[error("Player {0:?} tried to move a nonexistent piece at {1:?}")]
    EmptyMove(CrownfallPlayerKind, CrownfallBoardCell),
    #[error("Player {0:?} tried to move an enemy piece at {1:?}")]
    EnemyMove(CrownfallPlayerKind, CrownfallBoardCell),
    #[error("Player {0:?} tried to move from {1:?} to invalid destination {2:?}")]
    InvalidDestination(CrownfallPlayerKind, CrownfallBoardCell, CrownfallBoardCell),
    #[error("Player {0:?} must play a capturing move this turn")]
    CaptureRequired(CrownfallPlayerKind),
}
