# Crownfall

A simple board game.

## Rules

Crownfall is a two-player abstract strategy game played on a 7×7 board. Each player commands a Crown, six Knights, and three Spies, and must protect their own Crown while working to eliminate the opponent's forces.

### Components

- **Board:** 7×7 grid, columns 1–7 and rows A–G
- **Pieces (per player):**
    - 1 × Crown (👑)
    - 6 × Knight (🛡️)
    - 3 × Spy (🗡️)

### Setup

|       | 1  | 2  | 3  | 4  | 5  | 6  | 7  |
|-------|----|----|----|----|----|----|----|
| **A** |    |    | 🗡️️ | 👑 | 🗡️️ |    |    |
| **B** | 🛡️ | 🛡 | 🛡️ | 🗡️ | 🛡️ | 🛡️ | 🛡️ |
| **C** |    |    |    |    |    |    |    |
| **D** |    |    |    |    |    |    |    |
| **E** |    |    |    |    |    |    |    |
| **F** | 🛡️ | 🛡 | 🛡️ | 🗡️️ | 🛡️ | 🛡 | 🛡️ |
| **G** |    |    | 🗡️️ | 👑 | 🗡️️ |    |    |

Rows A–B belong to one player; rows F–G belong to the other.

### Turn Structure

White goes first.

1. Players alternate turns.
2. On a turn, a player must move exactly one of their own pieces one tile:
   - **Crown and Spies** move in an orthogonal direction (up, down, left, or right). Diagonal moves are not permitted.
   - **Knights** move forward-only, toward the opponent's starting rows, to any of the (up to) 3 cells in the row directly ahead of them: straight forward, forward-left, or forward-right. Knights cannot move sideways or backward.
3. A piece may not move onto an occupied tile or off the board.
4. After a move, any captures that result from the move are resolved immediately (see **Captures**). Capture adjacency is always orthogonal (see **Adjacency**), regardless of how a piece moved to get there.

### Adjacency

Two tiles are **adjacent** if they share an edge (orthogonal neighbors only (up, down, left, right)). Diagonal tiles are never adjacent.

**Surrounded** means a piece has enemy pieces occupying two of its orthogonally adjacent tiles that form a valid capturing pincer (see **Captures**), in any combination of positions (e.g., left+right, left+up, up+right, etc.) — not only opposite sides. Other enemy pieces also adjacent to the same tile do not prevent this.

### Captures

#### Knight Capture

Occurs when a knight is moved next to an enemy piece that already has 1 knight adjacent. The captured piece is removed, and if the enemy piece was a knight the attacking player must remove one of their knights.

#### Spy Capture

Occurs whenever two spies surround an enemy piece, even if the enemy moved there. The captured piece is removed.

#### Crown

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

### Losing the Game

A player loses immediately if either condition is met:

1. **Crown captured:** The Crown is surrounded by two enemy Spies, by two enemy Knights or by an enemy Knight and Crown. This check has the **highest priority** — if it is ever true, the game ends immediately before any other capture or condition is evaluated.
2. **Attrition:** The player has one or fewer Knights **and** one or fewer Spies remaining. Spy Capture works independently of Knights, so holding spies alone still keeps a player in the fight — attrition only applies once both are nearly gone.

### Draws

The game ends in a draw, with no winner, if either condition is met:

1. **Threefold repetition:** The same position (board layout and player to move) occurs three times over the course of the game.
2. **No progress:** 40 consecutive turns pass without a capture (the equivalent of chess's 50-move rule — Crownfall has no pawn-equivalent, so "no capture" is the sole progress signal).
3. **Turn limit:** The game reaches 200 total turns. This is an absolute safety net and applies regardless of repetition or recent captures.
4. **Mutual knight exhaustion:** A Knight Capture leaves one player with a single Knight and the other with none. Since the Knight Capture rule requires the attacker to also give up one of their own Knights when the captured piece was a Knight, this exchange can hit both sides in the same move — the game is ruled a draw rather than a win for whichever side still has a Knight.