//! Integration tests for the rule variants: Mini/Grand board layouts, the
//! Archer's ranged capture, mandatory capture, all-captures-processed, and
//! diagonal-moving Knights. Standard-rules behavior is already covered
//! exhaustively in `rules.rs` - this file only tests what's *new* per
//! variant.

use eb_crownfall_engine::errors::CrownfallError;
use eb_crownfall_engine::*;

// ---------------------------------------------------------------------
// Helpers (variant-aware versions of rules.rs's board-building helpers)
// ---------------------------------------------------------------------

fn place(
    board: &mut CrownfallBoardState,
    x: usize,
    y: usize,
    kind: CrownfallPieceKind,
    player: CrownfallPlayerKind,
) {
    let variant = board.variant();
    board.cells_mut()[CrownfallBoardCell::new_coord(x, y, variant).to_index()] =
        Some(CrownfallPiece { kind, player });
}

fn piece_at(board: &CrownfallBoardState, x: usize, y: usize) -> Option<CrownfallPiece> {
    let variant = board.variant();
    board.cells()[CrownfallBoardCell::new_coord(x, y, variant).to_index()]
}

fn game_with(
    board: CrownfallBoardState,
    player: CrownfallPlayerKind,
    rules: CrownfallRules,
) -> CrownfallGame {
    CrownfallGame {
        board,
        state: CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput { player }),
        rules,
        history: vec![0u64],
        moves_since_capture: 0,
    }
}

fn mv(
    game: &mut CrownfallGame,
    player: CrownfallPlayerKind,
    from: (usize, usize),
    to: (usize, usize),
) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
    let variant = game.board.variant();
    game.apply_action(CrownfallPlayerAction::Move {
        player,
        from: CrownfallBoardCell::new_coord(from.0, from.1, variant),
        to: CrownfallBoardCell::new_coord(to.0, to.1, variant),
    })
}

fn count(
    board: &CrownfallBoardState,
    player: CrownfallPlayerKind,
    kind: CrownfallPieceKind,
) -> usize {
    board
        .cells()
        .iter()
        .flatten()
        .filter(|p| p.player == player && p.kind == kind)
        .count()
}

use CrownfallPieceKind::{Archer, Crown, Knight, Spy};
use CrownfallPlayerKind::{Black, White};

// ---------------------------------------------------------------------
// Mini / Grand layouts
// ---------------------------------------------------------------------

#[test]
fn mini_layout_has_correct_board_size_and_piece_counts() {
    let game = CrownfallGame::new(CrownfallRules::mini());
    assert_eq!(game.board.board_length(), 5);
    assert_eq!(game.board.cells().len(), 25);
    for player in [White, Black] {
        assert_eq!(count(&game.board, player, Crown), 1);
        assert_eq!(count(&game.board, player, Knight), 4);
        assert_eq!(count(&game.board, player, Spy), 3);
        assert_eq!(count(&game.board, player, Archer), 0);
    }
}

#[test]
fn grand_layout_has_correct_board_size_and_piece_counts() {
    let game = CrownfallGame::new(CrownfallRules::grand());
    assert_eq!(game.board.board_length(), 9);
    assert_eq!(game.board.cells().len(), 81);
    for player in [White, Black] {
        assert_eq!(count(&game.board, player, Crown), 1);
        assert_eq!(count(&game.board, player, Knight), 8);
        assert_eq!(count(&game.board, player, Spy), 3);
        assert_eq!(count(&game.board, player, Archer), 2);
    }
}

#[test]
fn mini_board_movement_and_capture_still_work() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Mini);
    place(&mut board, 2, 2, Spy, White);
    let mut game = game_with(board, White, CrownfallRules::mini());
    let result = mv(&mut game, White, (2, 2), (2, 1)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 2, 1),
        Some(CrownfallPiece {
            kind: Spy,
            player: White
        })
    );
}

#[test]
fn grand_board_movement_and_capture_still_work() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 3, Spy, White); // target
    place(&mut board, 4, 4, Spy, Black); // pre-placed
    place(&mut board, 3, 2, Spy, Black); // will move to (4,2)
    let mut game = game_with(board, Black, CrownfallRules::grand());
    mv(&mut game, Black, (3, 2), (4, 2)).unwrap();
    assert_eq!(
        piece_at(&game.board, 4, 3),
        None,
        "spy captured across the 9x9 board"
    );
}

// ---------------------------------------------------------------------
// Board-scaled draw limits (Mini at half, Grand at double the Normal
// board's NO_PROGRESS_LIMIT/TOTAL_TURN_LIMIT)
// ---------------------------------------------------------------------

#[test]
fn mini_no_progress_draw_fires_at_half_the_normal_limit() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Mini);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 4, 4, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::mini());
    game.moves_since_capture = 19;

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.moves_since_capture, 20);
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::NoProgress));
}

#[test]
fn mini_no_progress_draw_does_not_fire_early() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Mini);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 4, 4, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::mini());
    game.moves_since_capture = 18;

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.moves_since_capture, 19);
    assert!(matches!(game.state, CrownfallGameState::Playing(_)));
}

#[test]
fn grand_no_progress_draw_fires_at_double_the_normal_limit() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 8, 8, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::grand());
    game.moves_since_capture = 79;

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.moves_since_capture, 80);
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::NoProgress));
}

#[test]
fn grand_no_progress_draw_does_not_fire_early() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 8, 8, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::grand());
    game.moves_since_capture = 78;

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.moves_since_capture, 79);
    assert!(matches!(game.state, CrownfallGameState::Playing(_)));
}

#[test]
fn mini_turn_limit_draw_fires_at_half_the_normal_limit() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Mini);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 4, 4, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::mini());
    game.history = vec![0u64; 100];

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::TurnLimit));
}

#[test]
fn grand_turn_limit_draw_fires_at_double_the_normal_limit() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 8, 8, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::grand());
    game.history = vec![0u64; 400];

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::TurnLimit));
}

#[test]
fn turns_remaining_scales_with_board_variant() {
    let mini = CrownfallGame::new(CrownfallRules::mini());
    assert_eq!(mini.turns_remaining(), 100);
    assert_eq!(mini.turns_remaining_before_no_progress_draw(), 20);

    let normal = CrownfallGame::new(CrownfallRules::standard());
    assert_eq!(normal.turns_remaining(), 200);
    assert_eq!(normal.turns_remaining_before_no_progress_draw(), 40);

    let grand = CrownfallGame::new(CrownfallRules::grand());
    assert_eq!(grand.turns_remaining(), 400);
    assert_eq!(grand.turns_remaining_before_no_progress_draw(), 80);
}

// ---------------------------------------------------------------------
// Archer
// ---------------------------------------------------------------------

/// "Sk A works as there is a spy touching the knight": an allied Crown/
/// Knight/Spy orthogonally adjacent to the target, plus the Archer landing
/// exactly 2 tiles away this turn, captures it.
#[test]
fn archer_ranged_capture_with_orthogonally_adjacent_ally() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 4, Knight, Black); // target
    place(&mut board, 3, 4, Spy, White); // ally, orthogonally adjacent to target
    place(&mut board, 6, 5, Archer, White); // will move to (6,4), 2 tiles from target
    let mut game = game_with(board, White, CrownfallRules::grand());

    let result = mv(&mut game, White, (6, 5), (6, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { player: White, removed, .. }) if removed.to_coord(CrownfallBoardVariant::Grand) == (4, 4)
    ));
    assert_eq!(
        piece_at(&game.board, 4, 4),
        None,
        "target hit by the archer"
    );
    assert_eq!(
        piece_at(&game.board, 3, 4),
        Some(CrownfallPiece {
            kind: Spy,
            player: White
        }),
        "ally isn't consumed"
    );
    assert_eq!(
        piece_at(&game.board, 6, 4),
        Some(CrownfallPiece {
            kind: Archer,
            player: White
        }),
        "archer doesn't move as part of firing"
    );
}

/// "S / k A doesn't work as the spy isn't ortho adjacent": a diagonally
/// adjacent ally doesn't satisfy the touching-ally condition.
#[test]
fn archer_ranged_capture_invalid_with_diagonally_adjacent_ally() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 4, Knight, Black); // target
    place(&mut board, 3, 3, Spy, White); // ally, only diagonally adjacent to target
    place(&mut board, 6, 5, Archer, White); // will move to (6,4), 2 tiles from target
    let mut game = game_with(board, White, CrownfallRules::grand());

    let result = mv(&mut game, White, (6, 5), (6, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 4, 4),
        Some(CrownfallPiece {
            kind: Knight,
            player: Black
        }),
        "target survives - the ally was never orthogonally adjacent"
    );
}

/// "Archers can not be used in any other capture, so CcA doesn't work":
/// an Archer standing where an ordinary pincer partner would go never
/// completes a Knight/Spy/Crown capture.
#[test]
fn archer_cannot_be_an_ordinary_pincer_partner() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 4, Crown, Black); // target
    place(&mut board, 3, 4, Crown, White); // ordinary ortho attacker
    place(&mut board, 5, 3, Archer, White); // will move to (5,4), ordinary ortho attacker position
    let mut game = game_with(board, White, CrownfallRules::grand());

    let result = mv(&mut game, White, (5, 3), (5, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 4, 4),
        Some(CrownfallPiece {
            kind: Crown,
            player: Black
        }),
        "Crown+Archer is never a valid capturing pair"
    );
}

/// The Archer's shot only evaluates when the Archer itself is the piece
/// that just moved - an already-in-place Archer doesn't fire just because
/// some other ally's move newly satisfies the adjacency condition.
#[test]
fn archer_shot_does_not_fire_from_a_different_pieces_move() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 4, Knight, Black); // target
    place(&mut board, 6, 4, Archer, White); // already in range, not moving this turn
    place(&mut board, 3, 5, Spy, White); // will move to (3,4), newly touching the target
    let mut game = game_with(board, White, CrownfallRules::grand());

    let result = mv(&mut game, White, (3, 5), (3, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 4, 4),
        Some(CrownfallPiece {
            kind: Knight,
            player: Black
        }),
        "the stationary archer doesn't fire off another piece's move"
    );
}

/// Archers are ordinary capture *targets* - a ordinary Spy Capture pincer
/// removes one just like any other non-Crown piece.
#[test]
fn archer_can_be_captured_by_ordinary_rules() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Grand);
    place(&mut board, 4, 4, Archer, Black); // target
    place(&mut board, 4, 5, Spy, White); // pre-placed
    place(&mut board, 3, 3, Spy, White); // will move to (4, 3)
    let mut game = game_with(board, White, CrownfallRules::grand());

    mv(&mut game, White, (3, 3), (4, 3)).unwrap();
    assert_eq!(
        piece_at(&game.board, 4, 4),
        None,
        "archer captured by an ordinary spy pincer"
    );
}

// ---------------------------------------------------------------------
// Mandatory capture
// ---------------------------------------------------------------------

#[test]
fn mandatory_capture_rejects_a_non_capturing_move_when_a_capture_is_available() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Spy, Black); // target
    place(&mut board, 3, 4, Spy, White); // partner, pre-placed
    place(&mut board, 2, 2, Spy, White); // can move to (2,3) to complete the capture
    place(&mut board, 0, 0, Spy, White); // has only non-capturing moves available
    let mut game = game_with(board, White, CrownfallRules::standard_mandatory_capture());

    let err = mv(&mut game, White, (0, 0), (1, 0)).unwrap_err();
    assert!(matches!(err, CrownfallError::CaptureRequired(White)));
    assert_eq!(
        piece_at(&game.board, 0, 0),
        Some(CrownfallPiece {
            kind: Spy,
            player: White
        }),
        "the rejected move never applied"
    );
}

#[test]
fn mandatory_capture_allows_the_capturing_move_itself() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Spy, Black); // target
    place(&mut board, 3, 4, Spy, White); // partner, pre-placed
    place(&mut board, 2, 2, Spy, White); // will move to (2,3)
    place(&mut board, 0, 0, Spy, White); // has only non-capturing moves available
    let mut game = game_with(board, White, CrownfallRules::standard_mandatory_capture());

    let result = mv(&mut game, White, (2, 2), (2, 3)).unwrap();
    assert!(matches!(result, Some(CrownfallTurnResult::Capture { .. })));
    assert_eq!(piece_at(&game.board, 3, 3), None);
}

#[test]
fn mandatory_capture_allows_ordinary_moves_when_none_are_available() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 6, 6, Spy, Black);
    let mut game = game_with(board, White, CrownfallRules::standard_mandatory_capture());

    let result = mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
}

// ---------------------------------------------------------------------
// All-captures-processed
// ---------------------------------------------------------------------

/// The exact geometry `rules.rs`'s `enemy_spy_trap_takes_priority_over_the_
/// movers_own_capture` uses, under `all_captures_processed`: this time both
/// captures resolve instead of the enemy's trap pre-empting the mover's own
/// pincer completion.
#[test]
fn all_captures_processed_resolves_both_the_trap_and_the_movers_capture() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 2, Spy, Black); // would-be victim of white's pincer
    place(&mut board, 3, 1, Spy, White); // white's partner for capturing (3,2)
    place(&mut board, 2, 3, Spy, Black); // trap attacker 1
    place(&mut board, 4, 3, Spy, Black); // trap attacker 2
    place(&mut board, 3, 4, Spy, White); // will move to (3,3): trapped, but also orthogonal to (3,2)
    let mut game = game_with(
        board,
        White,
        CrownfallRules::standard_all_captures_processed(),
    );

    mv(&mut game, White, (3, 4), (3, 3)).unwrap();
    assert_eq!(
        piece_at(&game.board, 3, 3),
        None,
        "moved white spy captured by black's trap"
    );
    assert_eq!(
        piece_at(&game.board, 3, 2),
        None,
        "black spy ALSO captured - both captures process under this variant"
    );
    assert_eq!(
        piece_at(&game.board, 3, 1),
        Some(CrownfallPiece {
            kind: Spy,
            player: White
        }),
        "white's uninvolved partner spy is untouched"
    );
    assert_eq!(
        piece_at(&game.board, 2, 3),
        Some(CrownfallPiece {
            kind: Spy,
            player: Black
        }),
        "black's trap attackers are untouched (Spy Capture never sacrifices an attacker)"
    );
}

// ---------------------------------------------------------------------
// Variant 6: diagonal-moving, orthogonal-capturing Knights
// ---------------------------------------------------------------------

#[test]
fn diagonal_knight_moves_only_diagonally_forward() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Knight, White);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(3, 3, CrownfallBoardVariant::Normal),
            CrownfallRules::standard_diagonal_knights(),
        )
        .into_iter()
        .map(|c| c.to_coord(CrownfallBoardVariant::Normal))
        .collect();
    dests.sort();
    assert_eq!(
        dests,
        vec![(2, 2), (4, 2)],
        "only the two forward-diagonal cells"
    );
}

#[test]
fn diagonal_knight_straight_or_backward_move_is_rejected() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Knight, White);
    let mut game = game_with(board, White, CrownfallRules::standard_diagonal_knights());

    let straight = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(
        straight,
        CrownfallError::InvalidDestination(White, _, _)
    ));

    let sideways = mv(&mut game, White, (3, 3), (4, 3)).unwrap_err();
    assert!(matches!(
        sideways,
        CrownfallError::InvalidDestination(White, _, _)
    ));
}

/// The moved Knight lands in the exposed (left/right, non-straight) subset
/// of the orthogonal capture shape - mirrors the standard-rules diagonal
/// nuance, swapped.
#[test]
fn diagonal_knight_capture_valid_when_moved_knight_lands_left_or_right() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Knight, Black); // target
    place(&mut board, 3, 4, Knight, White); // straight-ahead partner, pre-placed (fine, stationary)
    place(&mut board, 1, 4, Knight, White); // will move diagonally to (2,3), left of target
    let mut game = game_with(board, White, CrownfallRules::standard_diagonal_knights());

    let result = mv(&mut game, White, (1, 4), (2, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { player: White, .. })
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None, "target captured");
    assert_eq!(
        piece_at(&game.board, 2, 3),
        None,
        "moved attacking knight sacrificed"
    );
    assert_eq!(
        piece_at(&game.board, 3, 4),
        Some(CrownfallPiece {
            kind: Knight,
            player: White
        }),
        "partner knight survives"
    );
}

/// The moved Knight landing straight-ahead of the target (the non-exposed
/// cell) never completes the pincer, even though a stationary partner may
/// sit in the exposed subset.
#[test]
fn diagonal_knight_capture_invalid_when_moved_knight_lands_straight_ahead() {
    let mut board = CrownfallBoardState::empty(CrownfallBoardVariant::Normal);
    place(&mut board, 3, 3, Knight, Black); // target
    place(&mut board, 2, 3, Knight, White); // left, pre-placed (exposed subset, but stationary)
    place(&mut board, 2, 5, Knight, White); // will move diagonally to (3,4), straight-ahead of target
    let mut game = game_with(board, White, CrownfallRules::standard_diagonal_knights());

    let result = mv(&mut game, White, (2, 5), (3, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 3, 3),
        Some(CrownfallPiece {
            kind: Knight,
            player: Black
        }),
        "target survives - the moved knight landed in the non-exposed straight-ahead cell"
    );
}
