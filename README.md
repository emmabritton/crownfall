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
2. On a turn, a player moves exactly one of their own pieces one tile in an orthogonal direction (up, down, left, or right). Diagonal moves are not permitted.
3. A piece may not move onto an occupied tile or off the board.
4. After a move, any captures that result from the move are resolved immediately (see **Captures**).

### Adjacency

Two tiles are **adjacent** if they share an edge (orthogonal neighbors only — up, down, left, right). Diagonal tiles are never adjacent.

**Surrounded** means a piece has enemy pieces occupying exactly two of its orthogonally adjacent tiles, in any combination of positions (e.g., left+right, left+up, up+right, etc.) — not only opposite sides.

### Captures

A capture is checked only at the moment the **moving player** completes a surround by moving a piece into position. A piece that moves itself into a surrounded position voluntarily is *not* captured — captures only trigger on the attacking player's move.

| Attackers | Target        | Result                                                   |
|-----------|---------------|----------------------------------------------------------|
| 2 Spies   | Knight or Spy | Target is removed. No attacker is lost.                  |
| 2 Knights | Knight        | Target is removed. One attacking Knight is also removed. |

The **Crown** may act as either a Knight or a Spy when participating in a capture, pairing with one real Knight or one real Spy:

- Crown + Spy surrounding a piece → resolves as a **Spy capture** (target removed, no losses).
- Crown + Knight surrounding a piece → resolves as a **Knight capture** (target removed, one Knight lost — the Crown is never the piece lost).

The Crown may use this ability to help capture *any* piece type, not just Knights (e.g., Crown + Knight can capture an enemy Spy).

Two Knights cannot capture a Spy or a Crown. Two Spies can capture a Knight, a Spy, or contribute to a Crown capture (see below).

### Losing the Game

A player loses immediately if either condition is met:

1. **Crown captured:** The Crown is surrounded by two enemy Spies, or by two enemy Knights. This check has the **highest priority** — if it is ever true, the game ends immediately before any other capture or condition is evaluated.
2. **Attrition:** The player has one or fewer Spies **and** one or fewer Knights remaining.