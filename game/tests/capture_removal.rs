use game::*;

fn empty_board() -> BoardState {
    BoardState {
        cells: [None; BOARD_LENGTH * BOARD_LENGTH],
    }
}

fn pad_board_to_avoid_attrition(board: &mut BoardState, player: PlayerKind) {
    board.cells[0] = Some(Piece { kind: PieceKind::Spy, player });
    board.cells[1] = Some(Piece { kind: PieceKind::Knight, player });
}

#[test]
fn spy_capture_removes_target_from_board() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::Black });
    board.cells[23] = Some(Piece { kind: PieceKind::Spy, player: PlayerKind::White });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece { kind: PieceKind::Spy, player: PlayerKind::White });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput { player: PlayerKind::White }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(game.board.cells[24].is_none(), "captured knight should be removed from the board");
}

#[test]
fn knight_capture_removes_target_and_one_attacker() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::Black });
    board.cells[23] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::White });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::White });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput { player: PlayerKind::White }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(game.board.cells[24].is_none(), "captured knight target should be removed from the board");
    let knight_losses = [game.board.cells[23], game.board.cells[25]]
        .iter()
        .filter(|c| c.is_none())
        .count();
    assert_eq!(knight_losses, 1, "exactly one attacking knight should be lost");
}

#[test]
fn crown_partnered_knight_capture_never_loses_the_crown() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::Black });
    board.cells[23] = Some(Piece { kind: PieceKind::Crown, player: PlayerKind::White });
    pad_board_to_avoid_attrition(&mut board, PlayerKind::Black);
    board.cells[32] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::White });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput { player: PlayerKind::White }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(32),
            to: Cell::new_index(25),
        })
        .expect("valid move");

    assert!(game.board.cells[24].is_none(), "captured target should be removed from the board");
    assert_eq!(
        game.board.cells[23],
        Some(Piece { kind: PieceKind::Crown, player: PlayerKind::White }),
        "the Crown should never be the piece lost"
    );
}

#[test]
fn single_move_capturing_two_pieces_removes_both() {
    let mut board = empty_board();
    board.cells[24] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::Black });
    board.cells[38] = Some(Piece { kind: PieceKind::Knight, player: PlayerKind::Black });
    board.cells[23] = Some(Piece { kind: PieceKind::Spy, player: PlayerKind::White });
    board.cells[39] = Some(Piece { kind: PieceKind::Spy, player: PlayerKind::White });
    board.cells[30] = Some(Piece { kind: PieceKind::Spy, player: PlayerKind::White });

    let game = Game {
        board,
        state: GameState::Playing(PlayState::WaitingForInput { player: PlayerKind::White }),
    };

    let (game, _result) = game
        .handle_player_action(PlayerAction::Move {
            player: PlayerKind::White,
            from: Cell::new_index(30),
            to: Cell::new_index(31),
        })
        .expect("valid move");

    assert!(game.board.cells[24].is_none(), "target A should be removed from the board");
    assert!(game.board.cells[38].is_none(), "target B should be removed from the board");
}
