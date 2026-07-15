# Crownfall

A two-player abstract strategy board game — protect your Crown, eliminate your opponent's pieces, played over a simple client/server desktop app.

---

## For Players

### Getting started

1. **Login** — enter a username. This is remembered between launches 
2. From the game list screen, either:
   - **Create game** — pick a board size and ruleset (see [Rule variants](#rule-variants)) and wait for another player to join, or
   - **Join** an existing pending game from the list, or
   - **Play vs AI** — pick a difficulty and personality (see [Playing vs the computer](#playing-vs-the-computer)), then a board size and ruleset, and play immediately with no server needed.
3. **Play** — click and drag one of your pieces to a highlighted destination tile to move it. Captures happen automatically when a move completes one (see [Captures](#captures) below).

### Rules

Crownfall is a two-player abstract strategy game played on a 7×7 board by default (other board sizes are available — see [Rule variants](#rule-variants)). Each player commands a Crown, six Knights, and three Spies, and must protect their own Crown while working to eliminate the opponent's forces.

#### Components

- **Board:** 7×7 grid, columns 1–7 and rows A–G
- **Pieces (per player):**
    - 1 × Crown (👑)
    - 6 × Knight (🛡️)
    - 3 × Spy (🗡️)

#### Setup

|       | 1  | 2  | 3  | 4  | 5  | 6  | 7  |
|-------|----|----|----|----|----|----|----|
| **A** |    |    | 🗡️️ | 👑 | 🗡️️ |    |    |
| **B** | 🛡️ | 🛡 | 🛡️ | 🗡️ | 🛡️ | 🛡 | 🛡️ |
| **C** |    | ️   |    |    |    | ️   |    |
| **D** |    |    |    |    |    |    |    |
| **E** |    | ️   |    |    |    |    |    |
| **F** | 🛡️ | 🛡 | 🛡️ | 🗡️️ | 🛡️ | 🛡 | 🛡️ |
| **G** |    |    | 🗡️️ | 👑 | 🗡️️ |    |    |

Rows A–C belong to one player; rows E–G belong to the other. Each side's 6 Knights are split across two rows (4 nearest their own Crown, 2 one row further forward, offset into the gaps) rather than one solid line — this staggering keeps the two sides' Knights from meeting as a single contiguous wall, which would otherwise put every Knight in a mutual pincer simultaneously on first contact.

#### Turn structure

White goes first.

1. Players alternate turns.
2. On a turn, a player must move exactly one of their own pieces one tile:
   - **Crown and Spies** move in an orthogonal direction (up, down, left, or right). Diagonal moves are not permitted.
   - **Knights** move orthogonally like every other piece, except they can never move backward (away from the opponent's starting rows) — forward, left, or right only. Diagonal moves are never a legal Knight move; the diagonal-forward shape instead defines their Knight Capture reach (see **Captures**).
3. A piece may not move onto an occupied tile or off the board.
4. After a move, any captures that result from the move are resolved immediately (see **Captures**). Capture adjacency is always orthogonal (see **Adjacency**), regardless of how a piece moved to get there.

#### Movement

Knights can move left, right and forwards. They take diagonally forward, and require another knight in of the target as well.

Spies and Crown can move left, right, up and down. They take in any direction.

#### Adjacency

Two tiles are **adjacent** if they share an edge (orthogonal neighbors only (up, down, left, right)). Diagonal tiles are never adjacent.

**Surrounded** means a piece has enemy pieces occupying two of its orthogonally adjacent tiles that form a valid capturing pincer (see **Captures**), in any combination of positions (e.g., left+right, left+up, up+right, etc.) — not only opposite sides. Other enemy pieces also adjacent to the same tile do not prevent this.

#### Captures

Lower case = white
Upper case = black

K = Knight
S = Spy
C = Crown

##### Knight Capture

Knights capture diagonally forwards, and they require that another knight is also front of the target.
When capturing another knight, one of the attackers is removed as well.

```
k                               k
K                               KK                                    K
 K   bottom knight moves up to      this triggers the capture leaving    (attacking knight is also removed)
 
s                               s
K                               KK                                    KK
 K   bottom knight moves up to      this triggers the capture leaving   
 
Kk                              Kk
                                K
K   bottom knight moves up to        as the second black knight isn't front of the white, nothing happens
```

When capturing a crown the pieces can be on any two adjacent squares, or diagonally if a knight is capturing, e.g
```
valid

Kc K                              KcK
     right most knight moves left     capturing the crown (crown capture)
     
Kc                            Kc
                                K
  K                                  capturing the crown (knight capture)
     bottom knight moves up      


invalid

K c                            Kc
   K                             K   
     left knight moves right       attempting to trigger a crown capture, but that requires two orthogonal pieces

```

##### Spy Capture

Occurs whenever two spies surround an enemy piece, even if the enemy moved there. The captured piece is removed.

Enemy spy capture is checked before yours, so with this setup:
```
s
 s
SS
    moving the left black spy up should trigger a spy capture of the right white spy, but the white spy capture takings priority leaving

s
 s
 S 
```

##### Crown

The Crown may act as a knight for captures, however it's capture takes priority. So if you have

s = black spy
C = white crown
K = white knight

```
 sK
s
 C
```
and the crown moves up to capture the top spy with the knight
```
 sK
sC

```
instead the spies capturing the crown takes priority and black wins.

##### Archer Capture

Only available under the [Archers rule variant](#rule-variants). An Archer captures at orthogonal range 2 (exactly two tiles away, in a straight line) instead of by pincer, provided an allied piece (any kind except another Archer) is orthogonally adjacent to the target. The shot only fires when the Archer itself is the piece that just moved — an Archer that was already in range before this turn doesn't fire. An Archer can never act as a pincer partner in a Knight or Spy Capture, but can be captured normally like any other piece.

#### Losing the game

A player loses immediately if either condition is met:

1. **Crown captured:** The Crown is surrounded by two enemy Spies, by two enemy Knights or by an enemy Knight and Crown, on any two of its four orthogonally adjacent tiles. Unlike an ordinary Knight Capture, a Crown capture is not bound by the Knight forward-arc restriction at all — a Knight on any side (even directly beside or behind the Crown) counts, and it doesn't matter which piece just moved. This check has the **highest priority** — if it is ever true, the game ends immediately before any other capture or condition is evaluated.
2. **Attrition:** The player has one or fewer Knights **and** one or fewer Spies remaining. Spy Capture works independently of Knights, so holding spies alone still keeps a player in the fight — attrition only applies once both are nearly gone. This condition is ignored entirely in the Archers variant: an Archer can still capture with no Knights or Spies left, so being reduced to Archers alone is not a loss.
3. **Surrender:** A player can concede the game at any time.

#### Draws

The game ends in a draw, with no winner, if either condition is met:

1. **Threefold repetition:** The same position (board layout and player to move) occurs three times over the course of the game.
2. **No progress:** 40 consecutive turns pass without a capture (the equivalent of chess's 50-move rule — Crownfall has no pawn-equivalent, so "no capture" is the sole progress signal). Doubled to 80 turns on the Grand board.
3. **Turn limit:** The game reaches 200 total turns. This is an absolute safety net and applies regardless of repetition or recent captures. Doubled to 400 turns on the Grand board.

A Knight Capture that leaves one player with a single Knight and the other with none (which can happen, since the attacker gives up one of their own Knights when the captured piece was a Knight) is not a special case — it's resolved by the normal Attrition and Draw rules above like any other position.

### Rule variants

When creating a game (or setting up a game against the AI) you can pick a board size and a ruleset independently, giving nine possible combinations. All the rules above describe the *standard* 7×7 ruleset unless stated otherwise.

#### Board sizes

| Board            | Size | Knights per side | Spies per side | Archers per side |
|------------------|------|------------------|----------------|------------------|
| Mini             | 5×5  | 3                | 2              | 0                |
| Normal (default) | 7×7  | 6                | 3              | 0                |
| Grand            | 9×9  | 8                | 3              | 2                |

The Mini and Grand boards use the same setup pattern as Normal (Knights staggered across two rows, Spies flanking the Crown), scaled to fit. No-progress and turn-limit draws scale with board size (doubled on Grand — see [Draws](#draws)).

#### Rulesets

- **Standard** — the rules described above, with no extra toggles.
- **Archers** — Replaces units with all Archers, 1 Spy and 1 Crown per side (see [Archer Capture](#archer-capture)) and disables the Attrition losing condition (see [Losing the game](#losing-the-game)).
- **Mandatory Capture** — if any legal move this turn would capture something, you must make a capturing move (any one of them, if there's a choice) rather than an ordinary move.
- **All Captures Processed** — normally, only the *first* applicable capture condition for a move is resolved (crown capture, then a pre-existing trap the mover walked into, then the mover's own capture). Under this variant, all of these are resolved simultaneously instead of the first one pre-empting the rest — so a move that both walks into an enemy trap *and* completes the mover's own capture resolves both.
- **Diagonal Knights** — inverts the Knight's movement/capture shapes: Knights move diagonally forward (instead of orthogonally) and their capture arc becomes orthogonal-forward/left/right (instead of diagonal-forward).

### Playing vs the computer

Choosing "Play vs AI" from the game list lets you set the computer opponent's:

- **Difficulty** (Easy / Medium / Hard / Very Hard) — how many turns ahead it searches. Higher difficulties see further into forced sequences but take longer to move.
- **Personality** (Defensive / Balanced / Aggressive) — how it weighs a position. Defensive values holding onto material and advances reluctantly; Aggressive discounts material slightly and leans on mobility and advancing toward your Crown, trading and pushing forward more readily.

AI games are entirely local — no server connection is used or required.

---

## For Developers

### Workspace layout

This is a Cargo workspace with four crates:

- **`eb-crownfall-engine/`** — pure game logic and state (no I/O). Board representation, moves, captures, turn resolution, and all rule variants live here. It's `#![no_std]` (see [no_std / GBA target](#no_std--gba-target)), so it has no dependency on the server/client at all — see [Using the engine as a library](#using-the-engine-as-a-library) to embed it in your own project.
- **`networking/`** — the shared TCP protocol used by both `server` and `client`: the `Packet` enum (login, list/create/join/poll/perform-action/leave a game, plus server-pushed updates) and wire types, length-prefixed JSON framing over a raw `TcpStream`, and thin non-blocking client/server wrappers.
- **`server/`** — a plain Rust binary (no async runtime) that accepts connections and dispatches `Packet`s against in-memory state. Nothing is persisted to disk — restarting the server drops all pending/active games.
- **`client/`** — a `pixels-graphics-lib`-based desktop app with a scene-based UI (login, game list, in-game board, AI settings, rules/game setup) that talks to the server over the same `Packet` protocol.

`eb-crownfall-engine` is the shared core that `networking`, `server`, and `client` all depend on; `networking` is the shared protocol that `server` and `client` both depend on.

### Building from source

Requires a recent stable Rust toolchain ([rustup.rs](https://rustup.rs) if you don't have one).

- Build everything: `cargo build`
- Run tests: `cargo test`
- Run tests for one crate: `cargo test --package eb-crownfall-engine`
- Check without building: `cargo check`

### Running your own server

```
cargo run --package server
```

Reads these environment variables (all optional):

- `PORT` — port to listen on (default `3000`)
- `ADDR` — address to bind (default `0.0.0.0`, all interfaces)
- `MAX_GAMES` — cap on concurrently active games (default `10`)
- `RUST_LOG` — log level for move/capture/game-over logging (see [Logging](#logging)); the server currently defaults to `warning` (quiet) unless overridden

State is entirely in-memory — restarting the server clears all games. There's no database or persistence layer.

Then point a client at it with the `LOCAL` environment variable set (see [What you need](#what-you-need)), or set `ADDR`/`PORT` and adjust the client's connection address in `client/src/main.rs` for a non-local server.

Deployment (of the public server) is via Railpack (`railpack.json`): it builds the server in release mode and runs the resulting binary directly.

### Using the engine as a library

`eb-crownfall-engine` has no dependency on the server or client — it's a plain library crate you can drive from any Rust program (including a `no_std` target, see below).

```rust
use eb_crownfall_engine::{
    CrownfallBoardCell, CrownfallGame, CrownfallPlayerAction, CrownfallPlayerKind, CrownfallRules,
};

// Pick a board size + ruleset. `CrownfallRules` has named constructors for
// the common combinations - `standard()`, `mini()`, `grand()`,
// `standard_archers()`, `standard_mandatory_capture()`, etc. - or build the
// struct directly for any other combination.
let game = CrownfallGame::new(CrownfallRules::standard());

// Find a legal destination for one of White's pieces.
let from_index = game
    .board
    .cells()
    .iter()
    .position(|c| matches!(c, Some(p) if p.player() == CrownfallPlayerKind::White))
    .unwrap();
let from = CrownfallBoardCell::new_index(from_index);
let to = game.board.get_valid_destinations_for(from, game.rules)[0];

// `handle_player_action` is the single entry point for making a move: it
// validates the action, applies it, and resolves any resulting
// captures/victory/draw, returning the new game state plus an optional
// summary of what happened this turn.
let action = CrownfallPlayerAction::Move { player: CrownfallPlayerKind::White, from, to };
let (game, turn_result) = game.handle_player_action(action)?;
```

Key types to know:

- `CrownfallRules { board: CrownfallBoardVariant, ruleset: CrownfallRuleset }` — the board size (`Mini`/`Normal`/`Grand`) and ruleset (`Archers`, or `Custom { mandatory_capture, all_captures_processed, knights_move_diagonally }`) a game is played with, independently combinable — see [Rule variants](#rule-variants) for what each one does.
- `CrownfallGame` — `board` + `rules` + `state` (`CrownfallGameState::Playing`/`Victory(player, WinReason)`/`Draw(DrawReason)`) + move history used for draw detection.
- `CrownfallBoardState` — the board itself; `cells()`/`cells_mut()` for direct access, `get_valid_destinations_for(cell, rules)` for legal-move generation.
- `CrownfallPlayerAction` — `Move`, `KnightRemoval` (choosing which Knight to sacrifice when a Knight Capture gives a choice), or `Surrender`.
- `CrownfallTurnResult` — what happened as a result of an action: `PieceMove`, `Capture`, or `Victory`.

The `ai` module (`eb_crownfall_engine::ai`) provides a negamax search you can call directly instead of writing your own opponent:

```rust
use eb_crownfall_engine::ai::{best_move, CrownfallDifficulty, CrownfallPersonality};

let action = best_move(&game, CrownfallPlayerKind::Black, CrownfallDifficulty::Medium.depth(), CrownfallPersonality::Balanced);
```

`eb-crownfall-engine/examples/simulate.rs` is a self-play harness (negamax vs itself) for balance analysis across many random-seeded games — useful for sanity-checking rule changes before committing to them:

```
cargo run --package eb-crownfall-engine --example simulate --release
```

### Logging

`eb-crownfall-engine` has an optional `log` feature (off by default) that emits one `log::info!` line per move, capture, Knight-Capture sacrifice, and final game-over reason, using `(x,y)` board coordinates. Both `server` and `client` enable this feature
