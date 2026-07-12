use game::*;

fn empty_board() -> BoardState {
    BoardState {
        cells: [None; BOARD_LENGTH * BOARD_LENGTH],
    }
}

fn pad_board_to_avoid_attrition(board: &mut BoardState, player: PlayerKind) {
    board.cells[0] = Some(Piece {
        kind: PieceKind::Spy,
        player,
    });
    board.cells[1] = Some(Piece {
        kind: PieceKind::Knight,
        player,
    });
}

#[test]
fn spy_capture_removes_target_from_board() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_none(),
        "captured knight should be removed from the board"
    );
}

#[test]
fn knight_capture_removes_target_and_one_attacker() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_none(),
        "captured knight target should be removed from the board"
    );
    let knight_losses = [game.board.cells[23], game.board.cells[25]]
        .iter()
        .filter(|c| c.is_none())
        .count();
    assert_eq!(
        knight_losses, 1,
        "exactly one attacking knight should be lost"
    );
}

#[test]
fn attrition_ignores_spy_count() {
    // Black is reduced to one knight by this capture but still holds three
    // spies. Attrition should trigger on knight count alone (README "Losing
    // the Game") — spy count must not save (or condemn) a player.
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[5] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[6] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[12] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[13] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    board.cells[32] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        GameState::Victory(PlayerKind::White),
        "black should lose to attrition on knight count alone, despite still holding three spies"
    );
}

#[test]
fn knight_pincer_capturing_a_spy_loses_no_knight() {
    // Two knights forming a valid pincer over a Spy (not a Knight) should capture
    // the spy without the README's knight-removal penalty, which only applies
    // when the captured piece was itself a Knight.
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_none(),
        "captured spy should be removed from the board"
    );
    assert!(
        game.board.cells[23].is_some() && game.board.cells[25].is_some(),
        "no attacking knight should be lost when the captured piece was a Spy"
    );
}

#[test]
fn crown_partnered_knight_capture_never_loses_the_crown() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Crown,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_none(),
        "captured target should be removed from the board"
    );
    assert_eq!(
        game.board.cells[23],
        Some(Piece {
            kind: PieceKind::Crown,
            player: PlayerKind::White
        }),
        "the Crown should never be the piece lost"
    );
}

#[test]
fn single_move_capturing_two_pieces_removes_both() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[38] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });
    board.cells[39] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });
    board.cells[30] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(30),
            to: Cell::new_index(31),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_none(),
        "target A should be removed from the board"
    );
    assert!(
        game.board.cells[38].is_none(),
        "target B should be removed from the board"
    );
}

#[test]
fn extra_adjacent_attacker_does_not_block_a_valid_pincer() {
    // A third attacker-owned piece (of any kind) is also adjacent to the target,
    // alongside two knights that form a valid pincer. The extra piece must not
    // prevent the knight capture from resolving.
    let mut board = empty_board();
    board.cells[23] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[30] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    board.cells[22] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });
    board.cells[31] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(31),
            to: Cell::new_index(24),
        })
        .expect("valid move");

    assert!(
        game.board.cells[23].is_none(),
        "the black knight should be captured despite a third white piece also being adjacent"
    );
    assert!(matches!(
        result,
        Some(TurnResult::Capture { removed, .. }) if removed == Cell::new_index(23)
    ));
}

#[test]
fn crown_cannot_stand_in_for_a_spy() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::Black,
    });
    board.cells[23] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::White,
    });
    board.cells[32] = Some(Piece {
        kind: PieceKind::Crown,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(
        game.board.cells[24].is_some(),
        "a Spy+Crown pair is not a valid capture — the Crown may only stand in for a Knight"
    );
}

#[test]
fn moving_into_a_spy_pincer_captures_the_moved_piece() {
    let mut board = empty_board();
    board.cells[7] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[21] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[15] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::White);

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(15),
            to: Cell::new_index(14),
        })
        .expect("valid move");

    assert!(
        game.board.cells[14].is_none(),
        "the moved knight walked into a Spy pincer and should be captured, even though it moved there itself"
    );
    assert!(
        matches!(result, Some(TurnResult::Capture { removed, .. }) if removed == Cell::new_index(14))
    );
}

#[test]
fn crown_walking_into_a_pincer_loses_immediately_even_mid_capture() {
    // README "Crown" worked example: the crown moves up to pair with its own knight
    // and capture a spy, but landing there also surrounds the crown with the two
    // black spies — the crown's own capture loses instantly, taking priority over the
    // capture it was attempting.
    let mut board = empty_board();
    board.cells[1] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[7] = Some(Piece {
        kind: PieceKind::Spy,
        player: PlayerKind::Black,
    });
    board.cells[2] = Some(Piece {
        kind: PieceKind::Knight,
        player: PlayerKind::White,
    });
    board.cells[15] = Some(Piece {
        kind: PieceKind::Crown,
        player: PlayerKind::White,
    });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput {
            player: PlayerKind::White,
        }),
    };

    let (game, result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(15),
            to: Cell::new_index(8),
        })
        .expect("valid move");

    assert_eq!(
        game.state,
        GameState::Victory(PlayerKind::Black),
        "black wins: the crown's own capture takes priority over the move it was attempting"
    );
    assert!(game.board.cells[8].is_none(), "the crown should be removed");
    assert!(
        game.board.cells[1].is_some(),
        "the spy the crown tried to capture survives"
    );
    assert!(
        matches!(result, Some(TurnResult::Victory { player:PlayerKind::White,surrounded_crown }) if surrounded_crown == Cell::new_index(8))
    );
}
