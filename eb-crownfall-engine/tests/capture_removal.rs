use eb_crownfall_engine::*;

fn empty_board() -> CrownfallBoardState {
    CrownfallBoardState::empty(CrownfallBoardVariant::Normal)
}

/// Every test here plays White to move under standard rules.
fn game_with(board: CrownfallBoardState) -> CrownfallGame {
    CrownfallGame::from_parts(
        board,
        CrownfallGameState::Playing(CrownfallPlayState::WaitingForInput {
            player: CrownfallPlayerKind::White,
        }),
        CrownfallRules::standard(),
    )
}

fn pad_board_to_avoid_attrition(board: &mut CrownfallBoardState, player: CrownfallPlayerKind) {
    board.cells_mut()[0] = Some(CrownfallPiece::new(CrownfallPieceKind::Spy, player));
    board.cells_mut()[1] = Some(CrownfallPiece::new(CrownfallPieceKind::Knight, player));
}

#[test]
fn spy_capture_removes_target_from_board() {
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(32),
            to: CrownfallBoardCell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "captured knight should be removed from the board"
    );
}

#[test]
fn knight_pincer_of_two_diagonal_attackers_captures() {
    // Both attackers diagonally ahead of the target, with the straight-ahead cell
    // empty — target (3,3)=24, attackers at (2,4)=30 and (4,4)=32.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[30] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "two diagonally-ahead knights should form a valid pincer even with the straight-ahead cell empty"
    );
}

#[test]
fn knight_beside_target_cannot_complete_a_pincer() {
    // One attacker directly ahead of the target (valid), the other directly beside it
    // (same row — not in either attacker's forward arc, invalid). Even though one
    // attacker is legitimately positioned, the side attacker disqualifies the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[38] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(38),
            to: CrownfallBoardCell::new_index(31),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_some(),
        "a side attacker plus a front attacker is not a valid Knight Capture pincer"
    );
    assert!(!matches!(result, Some(CrownfallTurnResult::Capture { .. })));
}

#[test]
fn knight_moving_directly_ahead_cannot_complete_a_pincer_even_with_diagonal_partner() {
    // Target Black Knight at (3,3)=24. A White Knight already sits diagonally ahead
    // at (4,4)=32. White moves a second Knight from (3,5)=38 to (3,4)=31 — directly
    // ahead of the target. Even though the resulting pair (31 direct + 32 diagonal)
    // is the same shape as `knight_capture_removes_target_and_one_attacker`, here it's
    // the *directly-ahead* Knight that just moved, not the diagonal one — so it must
    // NOT spring the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[38] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(38),
            to: CrownfallBoardCell::new_index(31),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_some(),
        "a Knight that just moved directly ahead of the target cannot spring a Knight Capture pincer"
    );
    assert!(!matches!(result, Some(CrownfallTurnResult::Capture { .. })));
}

#[test]
fn knight_capture_removes_target_and_one_attacker() {
    // Target Black Knight at (3,3)=24. White's forward-arc attacker cells against
    // it are the row "behind" it from White's perspective, y=4: (2,4)=30, (3,4)=31,
    // (4,4)=32. One White Knight already sits at 31 (directly ahead); the other
    // moves in from (4,5)=39 to 32 (diagonally ahead) to complete the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "captured knight target should be removed from the board"
    );
    let knight_losses = [game.board.cells()[31], game.board.cells()[32]]
        .iter()
        .filter(|c| c.is_none())
        .count();
    assert_eq!(
        knight_losses, 1,
        "exactly one attacking knight should be lost"
    );
}

#[test]
fn high_spy_count_prevents_attrition_despite_low_knight_count() {
    // Black is reduced to one knight by this capture but still holds three
    // spies. Attrition requires both Knights and Spies to be depleted
    // (README "Losing the Game") — a strong spy count keeps a player in the
    // fight even after their knights are nearly gone.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[5] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[6] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[12] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[13] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        matches!(game.state, CrownfallGameState::Playing(_)),
        "black should not lose to attrition while still holding three spies, despite being down to one knight"
    );
}

#[test]
fn attrition_triggers_once_both_knights_and_spies_are_depleted() {
    // Black's only knight is captured by a White Spy+Spy pincer (not a
    // Knight Capture, so no self-penalty knight loss for White), and Black's
    // sole spy is untouched. With Black left at zero knights and one spy,
    // both thresholds are met and White wins by attrition.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[6] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(32),
            to: CrownfallBoardCell::new_index(25),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        CrownfallGameState::Victory(CrownfallPlayerKind::White, WinReason::Attrition),
        "black should lose to attrition once both knight and spy counts are at or below one"
    );
}

#[test]
fn knight_capture_leaving_one_knight_each_side_from_the_same_capture() {
    // Black holds only one knight; White captures it with a Knight+Knight
    // pincer, but per the Knight Capture rule the attacker must also give up
    // one of their own knights since the captured piece was a Knight. That
    // leaves White with one knight and Black with none — with no spies on
    // the board either, this is an ordinary attrition win for White.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        CrownfallGameState::Victory(CrownfallPlayerKind::White, WinReason::Attrition),
        "White ends with one knight and Black with none, both with no spies — an attrition win for White"
    );
}

#[test]
fn knight_pincer_capturing_a_spy_loses_no_knight() {
    // Two knights forming a valid pincer over a Spy (not a Knight) should capture
    // the spy without the README's knight-removal penalty, which only applies
    // when the captured piece was itself a Knight.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "captured spy should be removed from the board"
    );
    assert!(
        game.board.cells()[31].is_some() && game.board.cells()[32].is_some(),
        "no attacking knight should be lost when the captured piece was a Spy"
    );
}

#[test]
fn crown_partnered_knight_capture_never_loses_the_crown() {
    // Crown sits beside the target (23, a side cell — fine, the Crown isn't bound by
    // the Knight forward-arc restriction), while the Knight moves in from (4,5)=39 to
    // (4,4)=32, diagonally ahead of the target at (3,3)=24 — the just-moved Knight
    // must land diagonally (not directly) ahead to spring the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "captured target should be removed from the board"
    );
    assert_eq!(
        game.board.cells()[23],
        Some(CrownfallPiece::new(
            CrownfallPieceKind::Crown,
            CrownfallPlayerKind::White
        )),
        "the Crown should never be the piece lost"
    );
}

#[test]
fn knight_moving_directly_ahead_cannot_complete_a_pincer_even_with_crown_partner() {
    // Same setup as `crown_partnered_knight_capture_never_loses_the_crown`, except the
    // Knight moves in from (3,5)=38 to (3,4)=31, straight ahead of the target at
    // (3,3)=24, instead of diagonally. The Crown's own exemption from the forward-arc
    // restriction doesn't help here — it's the just-moved Knight landing directly
    // ahead, not diagonally, so it must not spring the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);
    board.cells_mut()[38] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(38),
            to: CrownfallBoardCell::new_index(31),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_some(),
        "a Knight that just moved directly ahead of the target cannot spring a Knight Capture pincer, even with a Crown partner"
    );
    assert!(!matches!(result, Some(CrownfallTurnResult::Capture { .. })));
}

#[test]
fn single_move_capturing_two_pieces_removes_both() {
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[38] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[30] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(30),
            to: CrownfallBoardCell::new_index(31),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "target A should be removed from the board"
    );
    assert!(
        game.board.cells()[38].is_none(),
        "target B should be removed from the board"
    );
}

#[test]
fn extra_adjacent_attacker_does_not_block_a_valid_pincer() {
    // A third attacker-owned piece (of any kind) is also adjacent to the target,
    // alongside two knights that form a valid pincer. The extra piece must not
    // prevent the knight capture from resolving.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::Black);

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_none(),
        "the black knight should be captured despite a third white piece also being adjacent"
    );
    assert!(matches!(
        result,
        Some(CrownfallTurnResult::Capture { removed, .. }) if removed == CrownfallBoardCell::new_index(24)
    ));
}

#[test]
fn crown_cannot_stand_in_for_a_spy() {
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, _result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(32),
            to: CrownfallBoardCell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_some(),
        "a Spy+Crown pair is not a valid capture — the Crown may only stand in for a Knight"
    );
}

#[test]
fn moving_into_a_spy_pincer_captures_the_moved_piece() {
    // The knight moves straight forward (White's only sideways-free option
    // now that Knights are forward-only) into a pre-existing Black Spy
    // pincer at 14/16.
    let mut board = empty_board();
    board.cells_mut()[14] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[16] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[22] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    pad_board_to_avoid_attrition(&mut board, CrownfallPlayerKind::White);

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(22),
            to: CrownfallBoardCell::new_index(15),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[15].is_none(),
        "the moved knight walked into a Spy pincer and should be captured, even though it moved there itself"
    );
    assert!(
        matches!(result, Some(CrownfallTurnResult::Capture { removed, .. }) if removed == CrownfallBoardCell::new_index(15))
    );
}

#[test]
fn knights_can_move_orthogonally_but_not_backward_or_diagonally() {
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    // Sideways (same row) is legal.
    assert!(
        game.clone()
            .handle_player_action(CrownfallPlayerAction::Move {
                player: CrownfallPlayerKind::White,
                from: CrownfallBoardCell::new_index(24),
                to: CrownfallBoardCell::new_index(23),
            })
            .is_ok()
    );

    // Backward (away from the opponent's side) is illegal.
    assert!(
        game.clone()
            .handle_player_action(CrownfallPlayerAction::Move {
                player: CrownfallPlayerKind::White,
                from: CrownfallBoardCell::new_index(24),
                to: CrownfallBoardCell::new_index(31),
            })
            .is_err()
    );

    // Forward-diagonal is no longer a legal move — that shape is now the
    // Knight's capture reach, not a movement destination.
    assert!(
        game.clone()
            .handle_player_action(CrownfallPlayerAction::Move {
                player: CrownfallPlayerKind::White,
                from: CrownfallBoardCell::new_index(24),
                to: CrownfallBoardCell::new_index(16),
            })
            .is_err()
    );

    // Straight forward is legal.
    let (game, _) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(24),
            to: CrownfallBoardCell::new_index(17),
        })
        .expect("straight-forward knight move should be legal");
    assert!(game.board.cells()[17].is_some());
}

#[test]
fn crown_walking_into_a_pincer_loses_immediately_even_mid_capture() {
    // README "Crown" worked example: the crown moves up to pair with its own knight
    // and capture a spy, but landing there also surrounds the crown with the two
    // black spies — the crown's own capture loses instantly, taking priority over the
    // capture it was attempting.
    let mut board = empty_board();
    board.cells_mut()[1] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[7] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[2] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[15] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(15),
            to: CrownfallBoardCell::new_index(8),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        CrownfallGameState::Victory(CrownfallPlayerKind::Black, WinReason::CrownCaptured),
        "black wins: the crown's own capture takes priority over the move it was attempting"
    );
    assert!(
        game.board.cells()[8].is_none(),
        "the crown should be removed"
    );
    assert!(
        game.board.cells()[1].is_some(),
        "the spy the crown tried to capture survives"
    );
    assert!(
        matches!(result, Some(CrownfallTurnResult::Victory { player:CrownfallPlayerKind::Black,surrounded_crown }) if surrounded_crown == CrownfallBoardCell::new_index(8))
    );
}

#[test]
fn crown_capture_needs_no_knight_arc_any_two_adjacent_sides_count() {
    // Crown captures are not bound by the Knight forward-arc restriction at all: a
    // Knight directly below the Crown (3,4)=31 plus a Knight moving in beside it, to
    // its right (4,3)=25, is enough — even though a Knight "beside" a target is never
    // a valid attacker for an ordinary Knight Capture (see
    // `knight_beside_target_cannot_complete_a_pincer`).
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[31] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(32),
            to: CrownfallBoardCell::new_index(25),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        CrownfallGameState::Victory(CrownfallPlayerKind::White, WinReason::CrownCaptured),
        "two knights on any two of the crown's sides, direct or beside, should capture it"
    );
    assert!(
        game.board.cells()[24].is_none(),
        "the crown should be removed"
    );
    assert!(
        matches!(result, Some(CrownfallTurnResult::Victory { player: CrownfallPlayerKind::White, surrounded_crown }) if surrounded_crown == CrownfallBoardCell::new_index(24))
    );
}

#[test]
fn crown_capture_via_a_knights_diagonal_reach_when_that_knight_just_moved() {
    // A Knight can attack the Crown from a diagonal cell (outside plain orthogonal
    // adjacency) if it's the piece that just moved there. Crown (Black) at (3,3)=24,
    // White Knight already beside it at (2,3)=23, and a second White Knight moves
    // from (4,5)=39 to (4,4)=32 — diagonally ahead of the Crown, not one of its
    // orthogonal neighbours — completing the pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(39),
            to: CrownfallBoardCell::new_index(32),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        CrownfallGameState::Victory(CrownfallPlayerKind::White, WinReason::CrownCaptured),
        "a Knight moving into a diagonal-forward cell of the Crown should complete the pincer"
    );
    assert!(
        game.board.cells()[24].is_none(),
        "the crown should be removed"
    );
    assert!(
        matches!(result, Some(CrownfallTurnResult::Victory { player: CrownfallPlayerKind::White, surrounded_crown }) if surrounded_crown == CrownfallBoardCell::new_index(24))
    );
}

#[test]
fn crown_capture_diagonal_reach_does_not_count_for_a_stationary_knight() {
    // Mirror of the previous test, but the diagonal Knight was already in place
    // *before* this move — it's a different Knight (the orthogonal one) that just
    // moved. Per the README "invalid" example, the diagonal reach only activates for
    // the actively-moving Knight, so this must NOT capture: only one orthogonal
    // attacker (the mover) is present, and one attacker alone can't complete a pincer.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[32] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[30] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let game = game_with(board);

    let (game, result) = game
        .handle_player_action(CrownfallPlayerAction::Move {
            player: CrownfallPlayerKind::White,
            from: CrownfallBoardCell::new_index(30),
            to: CrownfallBoardCell::new_index(23),
        })
        .expect("valid move");

    assert!(
        game.board.cells()[24].is_some(),
        "a Knight already sitting diagonally cannot complete a crown pincer unless it's the piece that just moved"
    );
    assert!(!matches!(result, Some(CrownfallTurnResult::Victory { .. })));
}

#[test]
fn preview_reports_an_ordinary_pincer_capture_without_touching_the_board() {
    // Same setup as `knight_pincer_of_two_diagonal_attackers_captures`.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[30] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[39] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    let before = board;

    let preview = board
        .preview_move_captures(
            CrownfallBoardCell::new_index(39),
            CrownfallBoardCell::new_index(32),
            CrownfallRules::standard(),
        )
        .expect("legal move");

    assert_eq!(preview.captured, vec![CrownfallBoardCell::new_index(24)]);
    assert!(!preview.mover_captured);
    assert_eq!(board, before, "preview must not mutate the board");
}

#[test]
fn preview_reports_the_mover_captured_by_a_pre_existing_spy_pincer() {
    // Same setup as `moving_into_a_spy_pincer_captures_the_moved_piece`.
    let mut board = empty_board();
    board.cells_mut()[14] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[16] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[22] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));

    let preview = board
        .preview_move_captures(
            CrownfallBoardCell::new_index(22),
            CrownfallBoardCell::new_index(15),
            CrownfallRules::standard(),
        )
        .expect("legal move");

    assert!(preview.captured.is_empty());
    assert!(
        preview.mover_captured,
        "the moved knight walks into a pre-existing Spy pincer"
    );
}

#[test]
fn preview_reports_the_crown_captured_by_walking_into_a_pincer() {
    // Same setup shape as `crown_walking_into_a_pincer_loses_immediately_even_mid_capture`
    // family: two enemy Spies flank the Crown's destination.
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Crown,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[16] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));
    board.cells_mut()[30] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));

    let preview = board
        .preview_move_captures(
            CrownfallBoardCell::new_index(24),
            CrownfallBoardCell::new_index(23),
            CrownfallRules::standard(),
        )
        .expect("legal move");

    assert!(
        preview.mover_captured,
        "the crown walks into a two-spy pincer"
    );
    assert!(
        preview.captured.is_empty(),
        "crown-loss takes priority over anything else and pre-empts it"
    );
}

#[test]
fn preview_is_none_for_an_illegal_destination() {
    let mut board = empty_board();
    board.cells_mut()[24] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Knight,
        CrownfallPlayerKind::White,
    ));
    board.cells_mut()[23] = Some(CrownfallPiece::new(
        CrownfallPieceKind::Spy,
        CrownfallPlayerKind::Black,
    ));

    // Occupied destination.
    assert!(
        board
            .preview_move_captures(
                CrownfallBoardCell::new_index(24),
                CrownfallBoardCell::new_index(23),
                CrownfallRules::standard(),
            )
            .is_none()
    );

    // No piece at `from`.
    assert!(
        board
            .preview_move_captures(
                CrownfallBoardCell::new_index(0),
                CrownfallBoardCell::new_index(1),
                CrownfallRules::standard(),
            )
            .is_none()
    );
}
