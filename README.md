# Crownfall

A simple board game.

## Rules

Crownfall is a two-player abstract strategy game played on a 7×7 board. Each player commands a Crown, five Knights, and four Spies, and must protect their own Crown while working to eliminate the opponent's forces.

### Components

- **Board:** 7×7 grid, columns 1–7 and rows A–G
- **Pieces (per player):**
    - 1 × Crown (👑)
    - 5 × Knight (🛡️)
    - 4 × Spy (🗡️)

### Setup

|       | 1  | 2  | 3  | 4  | 5  | 6  | 7  |
|-------|----|----|----|----|----|----|----|
| **A** |    |    | 🛡️ | 👑 | 🛡️ |    |    |
| **B** | 🗡️ | 🗡️ | 🛡️ | 🛡️ | 🛡️ | 🗡️ | 🗡️ |
| **C** |    |    |    |    |    |    |    |
| **D** |    |    |    |    |    |    |    |
| **E** |    |    |    |    |    |    |    |
| **F** | 🗡️ | 🗡️ | 🛡️ | 🛡️ | 🛡️ | 🗡️ | 🗡️ |
| **G** |    |    | 🛡️ | 👑 | 🛡️ |    |    |

Rows A–B belong to one player; rows F–G belong to the other.

### Turn Structure

White goes first.

1. Players alternate turns.
2. On a turn, a player must move exactly one of their own pieces one tile in an orthogonal direction (up, down, left, or right). Diagonal moves are not permitted.
3. A piece may not move onto an occupied tile or off the board.
4. After a move, any captures that result from the move are resolved immediately (see **Captures**).

### Adjacency

Two tiles are **adjacent** if they share an edge (orthogonal neighbors only (up, down, left, right)). Diagonal tiles are never adjacent.

**Surrounded** means a piece has enemy pieces occupying exactly two of its orthogonally adjacent tiles, in any combination of positions (e.g., left+right, left+up, up+right, etc.) — not only opposite sides.

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
2. **Attrition:** The player has one or fewer Spies **and** one or fewer Knights remaining.