//! Integration test suite for the Crownfall engine: board setup, movement
//! legality, every capture rule (Knight Capture, Spy Capture, Crown
//! Capture, self-traps and their priority ordering), attrition, mutual
//! knight exhaustion, and all four draw conditions.
//!
//! Most tests build a minimal custom board (`empty_board` + `place`)
//! rather than using the full default layout, so each scenario only
//! contains the pieces relevant to the rule under test and coordinates can
//! be reasoned about directly.

use eb_crownfall_engine::errors::CrownfallError;
use eb_crownfall_engine::*;

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

const VARIANT: CrownfallBoardVariant = CrownfallBoardVariant::Normal;

fn empty_board() -> CrownfallBoardState {
    CrownfallBoardState::empty(VARIANT)
}

fn place(
    board: &mut CrownfallBoardState,
    x: usize,
    y: usize,
    kind: CrownfallPieceKind,
    player: CrownfallPlayerKind,
) {
    board.cells_mut()[CrownfallBoardCell::new_coord(x, y, VARIANT).to_index()] =
        Some(CrownfallPiece::new(kind, player));
}

fn piece_at(board: &CrownfallBoardState, x: usize, y: usize) -> Option<CrownfallPiece> {
    board.cells()[CrownfallBoardCell::new_coord(x, y, VARIANT).to_index()]
}

fn game_with(board: CrownfallBoardState, player: CrownfallPlayerKind) -> CrownfallGame {
    let mut game = CrownfallGame::from_parts(
        board,
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput { player }),
        CrownfallRules::standard(),
    );
    game.history = vec![0u32];
    game
}

fn mv(
    game: &mut CrownfallGame,
    player: CrownfallPlayerKind,
    from: (usize, usize),
    to: (usize, usize),
) -> Result<Option<CrownfallTurnResult>, CrownfallError> {
    game.apply_action(CrownfallPlayerAction::Move {
        player,
        from: CrownfallBoardCell::new_coord(from.0, from.1, VARIANT),
        to: CrownfallBoardCell::new_coord(to.0, to.1, VARIANT),
    })
}

use CrownfallPieceKind::{Crown, Knight, Spy};
use CrownfallPlayerKind::{Black, White};

// ---------------------------------------------------------------------
// Board / cell basics
// ---------------------------------------------------------------------

#[test]
fn default_board_has_correct_piece_counts_and_positions() {
    let board = CrownfallBoardState::default();
    for player in [White, Black] {
        let crowns = board
            .cells()
            .iter()
            .flatten()
            .filter(|p| p.player() == player && p.kind() == Crown)
            .count();
        let knights = board
            .cells()
            .iter()
            .flatten()
            .filter(|p| p.player() == player && p.kind() == Knight)
            .count();
        let spies = board
            .cells()
            .iter()
            .flatten()
            .filter(|p| p.player() == player && p.kind() == Spy)
            .count();
        assert_eq!(crowns, 1, "{player:?} should have exactly 1 Crown");
        assert_eq!(knights, 6, "{player:?} should have exactly 6 Knights");
        assert_eq!(spies, 3, "{player:?} should have exactly 3 Spies");
    }
    // Crowns sit at the centre of the back row on each side.
    assert_eq!(
        piece_at(&board, 3, 0),
        Some(CrownfallPiece::new(Crown, Black))
    );
    assert_eq!(
        piece_at(&board, 3, 6),
        Some(CrownfallPiece::new(Crown, White))
    );
}

#[test]
fn default_game_starts_with_white_to_move() {
    let game = CrownfallGame::default();
    assert_eq!(
        game.state,
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput { player: White })
    );
    assert_eq!(game.moves_since_capture, 0);
}

#[test]
fn crownfall_player_kind_helpers() {
    assert_eq!(White.opposite(), Black);
    assert_eq!(Black.opposite(), White);
    assert_eq!(White.name(), "White");
    assert_eq!(Black.name(), "Black");
    assert_eq!(White.symbol(), 'W');
    assert_eq!(Black.symbol(), 'B');
}

#[test]
fn crownfall_piece_kind_helpers() {
    assert_eq!(Crown.name(), "Crown");
    assert_eq!(Knight.name(), "Knight");
    assert_eq!(Spy.name(), "Spy");
    assert_eq!(Crown.symbol(), 'C');
    assert_eq!(Knight.symbol(), 'K');
    assert_eq!(Spy.symbol(), 'S');
}

// ---------------------------------------------------------------------
// Movement legality
// ---------------------------------------------------------------------

#[test]
fn spy_and_crown_move_orthogonally_in_any_direction() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(3, 3, VARIANT),
            CrownfallRules::standard(),
        )
        .into_iter()
        .map(|c| c.to_coord(VARIANT))
        .collect();
    dests.sort();
    assert_eq!(dests, vec![(2, 3), (3, 2), (3, 4), (4, 3)]);
}

#[test]
fn crown_moves_like_a_spy() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, Black);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(3, 3, VARIANT),
            CrownfallRules::standard(),
        )
        .into_iter()
        .map(|c| c.to_coord(VARIANT))
        .collect();
    dests.sort();
    assert_eq!(dests, vec![(2, 3), (3, 2), (3, 4), (4, 3)]);
}

#[test]
fn knight_cannot_move_backward_or_diagonally() {
    let mut board = empty_board();
    // White advances toward y=0, so backward is +y.
    place(&mut board, 3, 3, Knight, White);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(3, 3, VARIANT),
            CrownfallRules::standard(),
        )
        .into_iter()
        .map(|c| c.to_coord(VARIANT))
        .collect();
    dests.sort();
    // Forward (3,2), left (2,3), right (4,3) - never backward (3,4), never diagonal.
    assert_eq!(dests, vec![(2, 3), (3, 2), (4, 3)]);
}

#[test]
fn black_knight_backward_is_toward_y_zero() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(3, 3, VARIANT),
            CrownfallRules::standard(),
        )
        .into_iter()
        .map(|c| c.to_coord(VARIANT))
        .collect();
    dests.sort();
    // Black forward is +y, so (3,2) [backward] is excluded.
    assert_eq!(dests, vec![(2, 3), (3, 4), (4, 3)]);
}

#[test]
fn pieces_cannot_move_onto_occupied_tiles() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    place(&mut board, 3, 2, Spy, Black);
    let dests = board.get_valid_destinations_for(
        CrownfallBoardCell::new_coord(3, 3, VARIANT),
        CrownfallRules::standard(),
    );
    assert!(!dests.iter().any(|c| c.to_coord(VARIANT) == (3, 2)));
}

#[test]
fn corner_piece_has_only_two_destinations() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    let mut dests: Vec<_> = board
        .get_valid_destinations_for(
            CrownfallBoardCell::new_coord(0, 0, VARIANT),
            CrownfallRules::standard(),
        )
        .into_iter()
        .map(|c| c.to_coord(VARIANT))
        .collect();
    dests.sort();
    assert_eq!(dests, vec![(0, 1), (1, 0)]);
}

// ---------------------------------------------------------------------
// apply_action: basic moves and errors
// ---------------------------------------------------------------------

#[test]
fn legal_move_updates_board_and_switches_turn() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    let mut game = game_with(board, White);
    let result = mv(&mut game, White, (3, 3), (3, 2)).unwrap();
    assert_eq!(
        result,
        Some(CrownfallTurnResult::PieceMove {
            player: White,
            from: CrownfallBoardCell::new_coord(3, 3, VARIANT),
            to: CrownfallBoardCell::new_coord(3, 2, VARIANT),
        })
    );
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        piece_at(&game.board, 3, 2),
        Some(CrownfallPiece::new(Spy, White))
    );
    assert_eq!(
        game.state,
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput { player: Black })
    );
}

#[test]
fn moving_out_of_turn_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, Black);
    let mut game = game_with(board, White);
    let err = mv(&mut game, Black, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(err, CrownfallError::NotYourTurn(Black)));
}

#[test]
fn moving_an_empty_cell_is_rejected() {
    let board = empty_board();
    let mut game = game_with(board, White);
    let err = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(err, CrownfallError::EmptyMove(White, _)));
}

#[test]
fn moving_an_enemy_piece_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, Black);
    let mut game = game_with(board, White);
    let err = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(err, CrownfallError::EnemyMove(White, _)));
}

#[test]
fn moving_to_a_non_adjacent_cell_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    let mut game = game_with(board, White);
    let err = mv(&mut game, White, (0, 0), (5, 5)).unwrap_err();
    assert!(matches!(
        err,
        CrownfallError::InvalidDestination(White, _, _)
    ));
}

#[test]
fn moving_onto_an_occupied_cell_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    place(&mut board, 3, 2, Spy, White);
    let mut game = game_with(board, White);
    let err = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(
        err,
        CrownfallError::InvalidDestination(White, _, _)
    ));
}

#[test]
fn knight_backward_move_is_rejected_as_invalid_destination() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, White);
    let mut game = game_with(board, White);
    let err = mv(&mut game, White, (3, 3), (3, 4)).unwrap_err();
    assert!(matches!(
        err,
        CrownfallError::InvalidDestination(White, _, _)
    ));
}

#[test]
fn acting_after_victory_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    let mut game = game_with(board, White);
    game.state = CrownfallGameState::Victory(White, WinReason::CrownCaptured);
    let err = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(err, CrownfallError::GameOver(White)));
}

#[test]
fn acting_after_draw_is_rejected() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    let mut game = game_with(board, White);
    game.state = CrownfallGameState::Draw(DrawReason::TurnLimit);
    let err = mv(&mut game, White, (3, 3), (3, 2)).unwrap_err();
    assert!(matches!(err, CrownfallError::GameOver(White)));
}

#[test]
fn surrender_gives_opponent_the_victory() {
    let board = empty_board();
    let mut game = game_with(board, White);
    let result = game
        .apply_action(CrownfallPlayerAction::Surrender { player: White })
        .unwrap();
    assert_eq!(result, None);
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(Black, WinReason::Surrender)
    );
}

#[test]
fn handle_player_action_is_equivalent_to_apply_action() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White);
    let game = game_with(board, White);
    let action = CrownfallPlayerAction::Move {
        player: White,
        from: CrownfallBoardCell::new_coord(3, 3, VARIANT),
        to: CrownfallBoardCell::new_coord(3, 2, VARIANT),
    };
    let (new_game, result) = game.handle_player_action(action).unwrap();
    assert_eq!(
        piece_at(&new_game.board, 3, 2),
        Some(CrownfallPiece::new(Spy, White))
    );
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
}

// ---------------------------------------------------------------------
// Knight Capture
// ---------------------------------------------------------------------

/// Valid Knight Capture: a pre-placed partner Knight sits directly ahead of
/// the target, and the moving Knight lands diagonally ahead of it this
/// turn. The moved Knight is also removed as the cost of the capture.
#[test]
fn knight_capture_valid_diagonal_arc() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // target
    place(&mut board, 3, 4, Knight, White); // straight-ahead partner, pre-placed
    place(&mut board, 2, 5, Knight, White); // will move to (2,4), diagonally ahead
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (2, 5), (2, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { player: White, removed, .. }) if removed.to_coord(VARIANT) == (3, 3)
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None, "target knight captured");
    assert_eq!(
        piece_at(&game.board, 2, 4),
        None,
        "moved attacking knight is sacrificed"
    );
    assert_eq!(
        piece_at(&game.board, 3, 4),
        Some(CrownfallPiece::new(Knight, White)),
        "partner knight survives"
    );
}

/// Two Knights orthogonally beside a target (not in its forward arc) can
/// never form a Knight Capture pincer, even if one of them is the piece
/// that just moved there.
#[test]
fn knight_capture_invalid_when_attackers_are_beside_target() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // target
    place(&mut board, 2, 3, Knight, White); // beside, pre-placed
    place(&mut board, 4, 4, Knight, White); // will move to (4,3), also beside
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (4, 4), (4, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 3, 3),
        Some(CrownfallPiece::new(Knight, Black)),
        "target survives - flanking knights aren't in the forward arc"
    );
}

/// A Crown paired with a Knight can capture from either orthogonal side
/// (unrestricted by the Knight forward-arc rule, since the mover isn't a
/// Knight). When the captured piece is itself a Knight, the sacrifice
/// falls on the moved Knight - never the Crown partner.
#[test]
fn crown_partnered_knight_capture_sacrifices_the_knight_not_the_crown() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // target
    place(&mut board, 2, 3, Crown, White); // partner, beside the target
    place(&mut board, 4, 5, Knight, White); // will move to (4,4), diagonally ahead
    let mut game = game_with(board, White);

    mv(&mut game, White, (4, 5), (4, 4)).unwrap();
    assert_eq!(piece_at(&game.board, 3, 3), None, "target captured");
    assert_eq!(
        piece_at(&game.board, 4, 4),
        None,
        "moved knight sacrificed"
    );
    assert_eq!(
        piece_at(&game.board, 2, 3),
        Some(CrownfallPiece::new(Crown, White)),
        "crown survives the trade"
    );
}

/// The Crown can only ever be the *stationary* partner in a pincer: a
/// pincer position the Crown itself completes by moving springs nothing -
/// the Crown never initiates a capture.
#[test]
fn crown_moving_into_pincer_position_does_not_capture() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // would-be target
    place(&mut board, 3, 4, Knight, White); // partner already in place
    place(&mut board, 2, 4, Crown, White); // will move to (2,3), beside the target
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (2, 4), (2, 3)).unwrap();
    assert_eq!(
        piece_at(&game.board, 3, 3),
        Some(CrownfallPiece::new(Knight, Black)),
        "a pincer completed by the Crown moving must not capture"
    );
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
}

/// The Knight-Capture pincer geometry (Crown+Knight) can also capture a
/// non-Knight target; in that case no attacker is sacrificed, since the
/// sacrifice rule only fires when the *captured* piece is a Knight.
#[test]
fn crown_partnered_capture_of_a_spy_has_no_sacrifice() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, Black); // target (not a Knight)
    place(&mut board, 2, 3, Crown, White); // partner, beside the target
    place(&mut board, 4, 5, Knight, White); // will move to (4,4), diagonally ahead
    let mut game = game_with(board, White);

    mv(&mut game, White, (4, 5), (4, 4)).unwrap();
    assert_eq!(piece_at(&game.board, 3, 3), None, "spy captured");
    assert_eq!(
        piece_at(&game.board, 4, 4),
        Some(CrownfallPiece::new(Knight, White)),
        "no sacrifice when the target wasn't a Knight"
    );
    assert_eq!(
        piece_at(&game.board, 2, 3),
        Some(CrownfallPiece::new(Crown, White))
    );
}

// ---------------------------------------------------------------------
// Spy Capture
// ---------------------------------------------------------------------

#[test]
fn spy_capture_of_a_spy_removes_target_with_no_sacrifice() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White); // target
    place(&mut board, 3, 4, Spy, Black); // pre-placed
    place(&mut board, 2, 2, Spy, Black); // will move to (2,3)
    let mut game = game_with(board, Black);

    let result = mv(&mut game, Black, (2, 2), (2, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { player: Black, removed, .. }) if removed.to_coord(VARIANT) == (3, 3)
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        piece_at(&game.board, 3, 4),
        Some(CrownfallPiece::new(Spy, Black))
    );
    assert_eq!(
        piece_at(&game.board, 2, 3),
        Some(CrownfallPiece::new(Spy, Black))
    );
}

/// Spy Capture works against any non-Crown piece, not only Spies.
#[test]
fn spy_capture_of_a_knight() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, White); // target
    place(&mut board, 3, 4, Spy, Black);
    place(&mut board, 2, 2, Spy, Black);
    let mut game = game_with(board, Black);

    mv(&mut game, Black, (2, 2), (2, 3)).unwrap();
    assert_eq!(piece_at(&game.board, 3, 3), None);
}

/// Two Spies orthogonally adjacent to the Crown are just as valid a
/// crown-capturing pair as two Knights (README: "surrounded by two enemy
/// Spies, by two enemy Knights, or by an enemy Knight and Crown").
#[test]
fn crown_capture_via_two_spies() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, White); // target
    place(&mut board, 3, 4, Spy, Black); // pre-placed, orthogonal
    place(&mut board, 2, 2, Spy, Black); // will move to (2,3), orthogonal
    let mut game = game_with(board, Black);

    let result = mv(&mut game, Black, (2, 2), (2, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Victory { player: Black, .. })
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(Black, WinReason::CrownCaptured)
    );
}

// ---------------------------------------------------------------------
// Crown Capture
// ---------------------------------------------------------------------

/// Any two orthogonal sides of the Crown complete a capture unconditionally
/// - it doesn't matter which of the two attackers just moved.
#[test]
fn crown_capture_via_two_orthogonal_attackers() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, Black); // target
    place(&mut board, 2, 3, Knight, White); // pre-placed, orthogonal
    place(&mut board, 4, 4, Knight, White); // will move to (4,3), orthogonal
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (4, 4), (4, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Victory { player: White, surrounded_crown }) if surrounded_crown.to_coord(VARIANT) == (3, 3)
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(White, WinReason::CrownCaptured)
    );
}

/// A Knight that just moved diagonally ahead of the Crown counts as a
/// second attacker even though it isn't orthogonally adjacent.
#[test]
fn crown_capture_via_ortho_plus_moved_diagonal_knight() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, Black); // target
    place(&mut board, 3, 2, Knight, White); // orthogonal, pre-placed
    place(&mut board, 2, 5, Knight, White); // will move to (2,4), diagonally ahead
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (2, 5), (2, 4)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Victory { player: White, .. })
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None);
}

/// A Knight sitting diagonally ahead of the Crown before this turn (not the
/// piece that just moved) does not count as an attacker - only one
/// orthogonal attacker exists, which isn't enough on its own.
#[test]
fn crown_capture_invalid_when_diagonal_knight_did_not_just_move() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, Black); // target
    place(&mut board, 2, 4, Knight, White); // diagonal, pre-placed (not moved this turn)
    place(&mut board, 4, 4, Knight, White); // will move to (4,3), orthogonal
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (4, 4), (4, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 3, 3),
        Some(CrownfallPiece::new(Crown, Black)),
        "a pre-existing diagonal knight doesn't activate the diagonal rule"
    );
}

/// A Spy + Knight pair is not a valid Crown-capturing combination.
#[test]
fn crown_capture_invalid_for_spy_and_knight_pair() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Crown, Black); // target
    place(&mut board, 3, 2, Spy, White); // pre-placed, orthogonal
    place(&mut board, 2, 4, Knight, White); // will move to (2,3), orthogonal
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (2, 4), (2, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::PieceMove { .. })
    ));
    assert_eq!(
        piece_at(&game.board, 3, 3),
        Some(CrownfallPiece::new(Crown, Black))
    );
}

// ---------------------------------------------------------------------
// Self-traps and priority ordering
// ---------------------------------------------------------------------

#[test]
fn own_crown_walking_into_a_pincer_loses_immediately() {
    let mut board = empty_board();
    place(&mut board, 3, 2, Knight, Black);
    place(&mut board, 2, 3, Knight, Black);
    place(&mut board, 3, 4, Crown, White); // will move to (3,3), into the trap
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (3, 4), (3, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Victory { player: Black, surrounded_crown }) if surrounded_crown.to_coord(VARIANT) == (3, 3)
    ));
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(Black, WinReason::CrownCaptured)
    );
    assert_eq!(piece_at(&game.board, 3, 3), None);
}

/// Own-crown-trap has the highest priority: even though the same move would
/// otherwise complete the crown's own Knight-Capture pincer of an adjacent
/// enemy Knight, the crown being captured takes precedence and the enemy
/// piece survives.
#[test]
fn own_crown_trap_takes_priority_over_the_movers_own_capture() {
    let mut board = empty_board();
    place(&mut board, 3, 2, Knight, Black); // trap attacker 1
    place(&mut board, 2, 3, Knight, Black); // trap attacker 2
    place(&mut board, 4, 3, Knight, Black); // would-be victim of white's pincer
    place(&mut board, 4, 4, Knight, White); // white's partner for capturing (4,3)
    place(&mut board, 3, 4, Crown, White); // will move to (3,3): trapped, but also orthogonal to (4,3)
    let mut game = game_with(board, White);

    mv(&mut game, White, (3, 4), (3, 3)).unwrap();
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(Black, WinReason::CrownCaptured)
    );
    assert_eq!(piece_at(&game.board, 3, 3), None, "white crown removed");
    assert_eq!(
        piece_at(&game.board, 4, 3),
        Some(CrownfallPiece::new(Knight, Black)),
        "black knight survives - the crown-trap pre-empted the capture check"
    );
}

#[test]
fn own_piece_walking_into_a_spy_pincer_is_captured() {
    let mut board = empty_board();
    place(&mut board, 2, 3, Spy, Black);
    place(&mut board, 4, 3, Spy, Black);
    place(&mut board, 3, 4, Knight, White); // will move to (3,3), into the trap
    let mut game = game_with(board, White);

    let result = mv(&mut game, White, (3, 4), (3, 3)).unwrap();
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { player: White, removed, .. }) if removed.to_coord(VARIANT) == (3, 3)
    ));
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        piece_at(&game.board, 2, 3),
        Some(CrownfallPiece::new(Spy, Black))
    );
    assert_eq!(
        piece_at(&game.board, 4, 3),
        Some(CrownfallPiece::new(Spy, Black))
    );
}

/// The enemy's pre-existing Spy trap on the square you move into is
/// resolved before your own would-be capture from that same move.
#[test]
fn enemy_spy_trap_takes_priority_over_the_movers_own_capture() {
    let mut board = empty_board();
    place(&mut board, 3, 2, Spy, Black); // would-be victim of white's pincer
    place(&mut board, 3, 1, Spy, White); // white's partner for capturing (3,2)
    place(&mut board, 2, 3, Spy, Black); // trap attacker 1
    place(&mut board, 4, 3, Spy, Black); // trap attacker 2
    place(&mut board, 3, 4, Spy, White); // will move to (3,3): trapped, but also orthogonal to (3,2)
    let mut game = game_with(board, White);

    mv(&mut game, White, (3, 4), (3, 3)).unwrap();
    assert_eq!(
        piece_at(&game.board, 3, 3),
        None,
        "moved white spy captured by black's trap"
    );
    assert_eq!(
        piece_at(&game.board, 3, 2),
        Some(CrownfallPiece::new(Spy, Black)),
        "black spy survives - white's own capture never got evaluated"
    );
    assert_eq!(
        piece_at(&game.board, 3, 1),
        Some(CrownfallPiece::new(Spy, White)),
        "white's uninvolved partner spy is untouched"
    );
}

// ---------------------------------------------------------------------
// Attrition
// ---------------------------------------------------------------------

#[test]
fn attrition_defeat_when_knights_and_spies_both_exhausted() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // black's only knight
    place(&mut board, 6, 6, Spy, Black); // black's only spy - stays at <=1 after capture
    place(&mut board, 3, 4, Spy, White);
    place(&mut board, 2, 2, Spy, White); // will move to (2,3)
    let mut game = game_with(board, White);

    mv(&mut game, White, (2, 2), (2, 3)).unwrap();
    assert_eq!(
        piece_at(&game.board, 3, 3),
        None,
        "black's last knight captured"
    );
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(White, WinReason::Attrition)
    );
}

#[test]
fn no_attrition_defeat_while_above_the_threshold() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // one of three black knights - this one gets captured
    place(&mut board, 1, 1, Knight, Black); // black's other knights survive
    place(&mut board, 1, 0, Knight, Black); // (kept off 0/1 to avoid the mutual-exhaustion pattern too)
    place(&mut board, 6, 6, Spy, Black);
    place(&mut board, 5, 5, Spy, Black); // black keeps 2 spies - above the <=1 threshold
    place(&mut board, 3, 4, Spy, White);
    place(&mut board, 2, 2, Spy, White); // will move to (2,3)
    let mut game = game_with(board, White);

    mv(&mut game, White, (2, 2), (2, 3)).unwrap();
    assert_eq!(piece_at(&game.board, 3, 3), None, "target captured");
    assert!(
        matches!(game.state, CrownfallGameState::Playing(_)),
        "black still has 2 knights + 2 spies"
    );
}

#[test]
fn knight_capture_leaving_one_knight_each_side_is_not_a_special_case() {
    // Black's only knight is captured while White sacrifices one of its
    // own two knights to make the capture - white ends with 1, black with 0.
    // With no spies on the board, black is also out of spies, so this is an
    // ordinary attrition win for White - there's no separate "mutual knight
    // exhaustion" draw rule anymore.
    let mut board = empty_board();
    place(&mut board, 3, 3, Knight, Black); // black's only knight
    place(&mut board, 3, 4, Knight, White); // white partner, survives
    place(&mut board, 2, 5, Knight, White); // white's other knight, sacrificed
    let mut game = game_with(board, White);

    mv(&mut game, White, (2, 5), (2, 4)).unwrap();
    assert_eq!(piece_at(&game.board, 3, 3), None);
    assert_eq!(
        piece_at(&game.board, 2, 4),
        None,
        "attacking knight sacrificed"
    );
    assert_eq!(
        game.state,
        CrownfallGameState::Victory(White, WinReason::Attrition)
    );
}

// ---------------------------------------------------------------------
// Draws
// ---------------------------------------------------------------------

#[test]
fn threefold_repetition_is_a_draw() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 6, 6, Spy, Black);
    let mut game = game_with(board, White);

    // Shuttle both spies out and back repeatedly. Every sub-position within
    // the cycle (not just the fully-reset one) recurs each time around, so
    // the draw can fire on any move once its own position hits 3 - stop as
    // soon as the game ends rather than assuming a fixed move count.
    let steps = [
        (White, (0, 0), (1, 0)),
        (Black, (6, 6), (5, 6)),
        (White, (1, 0), (0, 0)),
        (Black, (5, 6), (6, 6)),
    ];
    'outer: for _ in 0..3 {
        for &(player, from, to) in &steps {
            mv(&mut game, player, from, to).unwrap();
            if !matches!(game.state, CrownfallGameState::Playing(_)) {
                break 'outer;
            }
        }
    }

    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::Repetition));
}

#[test]
fn no_progress_draw_after_the_turn_limit_without_a_capture() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 6, 6, Spy, Black);
    let mut game = game_with(board, White);
    game.moves_since_capture = 39;

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.moves_since_capture, 40);
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::NoProgress));
}

#[test]
fn turn_limit_draw_as_an_absolute_safety_net() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 6, 6, Spy, Black);
    let mut game = game_with(board, White);
    game.history = vec![0u32; 200];

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.state, CrownfallGameState::Draw(DrawReason::TurnLimit));
}

#[test]
fn a_capture_resets_the_no_progress_counter() {
    let mut board = empty_board();
    place(&mut board, 3, 3, Spy, White); // target
    place(&mut board, 6, 6, Spy, White); // white's other spies, keep it out of attrition
    place(&mut board, 6, 5, Spy, White);
    place(&mut board, 3, 4, Spy, Black);
    place(&mut board, 2, 2, Spy, Black); // will move to (2,3)
    let mut game = game_with(board, Black);
    game.moves_since_capture = 39;

    mv(&mut game, Black, (2, 2), (2, 3)).unwrap();
    assert_eq!(game.moves_since_capture, 0);
    assert!(matches!(game.state, CrownfallGameState::Playing(_)));
}

#[test]
fn draw_reason_descriptions_are_stable() {
    assert_eq!(DrawReason::Repetition.description(), "threefold repetition");
    assert_eq!(
        DrawReason::NoProgress.description(),
        "no captures for too long"
    );
    assert_eq!(DrawReason::TurnLimit.description(), "turn limit reached");
}

#[test]
fn win_reason_descriptions_are_stable() {
    assert_eq!(WinReason::CrownCaptured.description(), "crown captured");
    assert_eq!(
        WinReason::Attrition.description(),
        "opponent out of knights and spies"
    );
    assert_eq!(WinReason::Surrender.description(), "opponent surrendered");
}

// ---------------------------------------------------------------------
// Turn/no-progress countdown helpers
// ---------------------------------------------------------------------

#[test]
fn turns_remaining_counts_down_from_the_turn_limit() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    place(&mut board, 6, 6, Spy, Black);
    let mut game = game_with(board, White);
    game.history = vec![0u32; 150];
    assert_eq!(game.turns_remaining(), 200 - 149);

    mv(&mut game, White, (0, 0), (1, 0)).unwrap();
    assert_eq!(game.turns_remaining(), 200 - 150);
}

#[test]
fn turns_remaining_before_no_progress_draw_counts_down() {
    let mut board = empty_board();
    place(&mut board, 0, 0, Spy, White);
    let mut game = game_with(board, White);
    assert_eq!(game.turns_remaining_before_no_progress_draw(), 40);
    game.moves_since_capture = 25;
    assert_eq!(game.turns_remaining_before_no_progress_draw(), 15);
}
