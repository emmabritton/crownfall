use crate::{Cell, PlayerKind};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GameError {
    #[error("Player {0:?} tried to remove an nonexistent knight at {1:?}")]
    EmptyKnightRemoval(PlayerKind, Cell),
    #[error("Player {0:?} tried to remove an enemy knight at {1:?}")]
    EnemyKnightRemoval(PlayerKind, Cell),
    #[error("Player {0:?} tried to act out of turn")]
    NotYourTurn(PlayerKind),
    #[error("Player {0:?} tried to act after the game had already ended")]
    GameOver(PlayerKind),
    #[error("Player {0:?} tried to move a nonexistent piece at {1:?}")]
    EmptyMove(PlayerKind, Cell),
    #[error("Player {0:?} tried to move an enemy piece at {1:?}")]
    EnemyMove(PlayerKind, Cell),
    #[error("Player {0:?} tried to move from {1:?} to invalid destination {2:?}")]
    InvalidDestination(PlayerKind, Cell, Cell),
}
