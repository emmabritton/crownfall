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
| **B** | 🛡️ | 🛡 | 🛡️ | 🗡️ | 🛡️ | 🛡 | 🛡️ |
| **C** |    | ️   |    |    |    | ️   |    |
| **D** |    |    |    |    |    |    |    |
| **E** |    | ️   |    |    |    |    |    |
| **F** | 🛡️ | 🛡 | 🛡️ | 🗡️️ | 🛡️ | 🛡 | 🛡️ |
| **G** |    |    | 🗡️️ | 👑 | 🗡️️ |    |    |

Rows A–C belong to one player; rows E–G belong to the other. Each side's 6 Knights are split across two rows (4 nearest their own Crown, 2 one row further forward, offset into the gaps) rather than one solid line — this staggering keeps the two sides' Knights from meeting as a single contiguous wall, which would otherwise put every Knight in a mutual pincer simultaneously on first contact.

### Turn Structure

White goes first.

1. Players alternate turns.
2. On a turn, a player must move exactly one of their own pieces one tile:
   - **Crown and Spies** move in an orthogonal direction (up, down, left, or right). Diagonal moves are not permitted.
   - **Knights** move orthogonally like every other piece, except they can never move backward (away from the opponent's starting rows) — forward, left, or right only. Diagonal moves are never a legal Knight move; the diagonal-forward shape instead defines their Knight Capture reach (see **Captures**).
3. A piece may not move onto an occupied tile or off the board.
4. After a move, any captures that result from the move are resolved immediately (see **Captures**). Capture adjacency is always orthogonal (see **Adjacency**), regardless of how a piece moved to get there.

### Movement

Knights can move left, right and forwards. They take diagonally forward, and require another knight in of the target as well.

Spies and Crown can move left, right, up and down. They take in any direction.

### Adjacency

Two tiles are **adjacent** if they share an edge (orthogonal neighbors only (up, down, left, right)). Diagonal tiles are never adjacent.

**Surrounded** means a piece has enemy pieces occupying two of its orthogonally adjacent tiles that form a valid capturing pincer (see **Captures**), in any combination of positions (e.g., left+right, left+up, up+right, etc.) — not only opposite sides. Other enemy pieces also adjacent to the same tile do not prevent this.

### Captures

Lower case = white
Upper case = black

K = Knight
S = Spy
C = Crown

#### Knight Capture

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

#### Spy Capture

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

1. **Crown captured:** The Crown is surrounded by two enemy Spies, by two enemy Knights or by an enemy Knight and Crown, on any two of its four orthogonally adjacent tiles. Unlike an ordinary Knight Capture, a Crown capture is not bound by the Knight forward-arc restriction at all — a Knight on any side (even directly beside or behind the Crown) counts, and it doesn't matter which piece just moved. This check has the **highest priority** — if it is ever true, the game ends immediately before any other capture or condition is evaluated.
2. **Attrition:** The player has one or fewer Knights **and** one or fewer Spies remaining. Spy Capture works independently of Knights, so holding spies alone still keeps a player in the fight — attrition only applies once both are nearly gone.

### Draws

The game ends in a draw, with no winner, if either condition is met:

1. **Threefold repetition:** The same position (board layout and player to move) occurs three times over the course of the game.
2. **No progress:** 40 consecutive turns pass without a capture (the equivalent of chess's 50-move rule — Crownfall has no pawn-equivalent, so "no capture" is the sole progress signal).
3. **Turn limit:** The game reaches 200 total turns. This is an absolute safety net and applies regardless of repetition or recent captures.
4. **Mutual knight exhaustion:** A Knight Capture leaves one player with a single Knight and the other with none. Since the Knight Capture rule requires the attacker to also give up one of their own Knights when the captured piece was a Knight, this exchange can hit both sides in the same move — the game is ruled a draw rather than a win for whichever side still has a Knight.

## Playing vs the Computer

Choosing "Play vs AI" from the game list lets you set the computer opponent's:

- **Difficulty** (Easy / Medium / Hard / Very Hard) — how many turns ahead it searches. Higher difficulties see further into forced sequences but take longer to move.
- **Personality** (Defensive / Balanced / Aggressive) — how it weighs a position. Defensive values holding onto material and advances reluctantly; Aggressive discounts material slightly and leans on mobility and advancing toward your Crown, trading and pushing forward more readily.