# Leaderboule Elo System — Pétanque

This document describes the custom Elo rating system implemented in `src/server/elo.rs`.

The design targets a club-scale Pétanque ladder:

- matches are played to 13 points, but an unfinished match may be recorded as long as the winner has more points than the loser;
- teams can contain 1 to 6 players on each side;
- score margins matter, but a blowout must not become a runaway rating generator;
- a 13–12 win must still be worth points;
- team composition must matter, especially when one strong player carries weaker teammates;
- the system must remain easy to understand and to display as integer rating changes.

The implementation is deterministic, server-side only, and uses the ratings stored in PostgreSQL as the sole source of truth. The client never submits its own rating values.

---

## 1. Notation

| Symbol | Meaning |
|---|---|
| `R_i` | Current rating of player `i` |
| `R_A^eff` | Effective rating of team A |
| `n_A` | Number of players in team A |
| `μ_A` | Arithmetic mean rating of team A |
| `σ_A` | Population standard deviation of team A |
| `E_A` | Probability that team A wins |
| `E_i` | Individual expected score of player `i` against the opposing team |
| `S_A` | Team result: `1` for a win, `0` for a loss |
| `M` | Margin-of-victory multiplier |
| `K_match` | Stakes for the whole match |
| `P_A` | Team rating pool of team A |
| `w_i` | Share of the team pool assigned to player `i` |
| `Δ_i` | Final integer rating change of player `i` |
| `B` | Completion bonus per player |

The examples in this report use a conventional 1500 rating scale. Differences between ratings are what matter; the application’s default rating of 400 is simply an arbitrary origin.

---

## 2. Constants

| Constant | Value | Rationale |
|---|---:|---|
| `ELO_SCALE` | `400` | Standard Elo logistic scale. A 400-point gap gives about 90.9% expected score. |
| `K_BASE` | `200` | A 1v1 even match at `M = 1` exchanges 100 points. |
| `MOV_REFERENCE` | `5` | 13–8 is the reference “typical” margin and gets `M = 1`. |
| `MOV_EXPONENT` | `0.7` | Concave curve: margins matter, but not proportionally. |
| `MOV_MIN` | `0.5` | Even a 13–12 win keeps a meaningful positive stake. |
| `MOV_MAX` | `1.9` | Caps the fanny / extreme-blowout multiplier. |
| `DISPERSION_WEIGHT` | `0.25` | Adds 25% of a team’s rating standard deviation to its effective strength. |
| `DISPERSION_CAP` | `200` | Prevents extreme rosters from receiving absurd effective ratings. |
| `INDIVIDUAL_WEIGHT_BLEND` | `0.65` | 65% individual expectation, 35% equal share. |
| `EXPECTATION_EPSILON` | `1e-6` | Guarantees strictly positive winner pools and strictly negative loser pools. |
| `COMPLETION_BONUS` | `1` | Small bonus per player when the winner reaches 13. Set to `0` for strict zero-sum. |
| `PLACEMENT_MAX_BONUS` | `0.5` | Up to 50% higher stakes when all participants are new. |
| `PLACEMENT_DECAY_GAMES` | `5` | The placement bonus decays with average experience. |
| `DEFAULT_ELO` | `400` | Existing application starting rating. |

---

## 3. Effective team rating

### 3.1 Mean and dispersion

For a team A of `n_A` players:

```text
μ_A = (1 / n_A) Σ R_i
σ_A = sqrt( (1 / n_A) Σ (R_i - μ_A)^2 )
```

`σ_A` is the population standard deviation. It is zero for a homogeneous team and grows when ratings are spread out.

### 3.2 Effective rating

```text
R_A^eff = μ_A + min(DISPERSION_CAP, DISPERSION_WEIGHT × σ_A)
```

A heterogeneous team receives a **higher** effective rating than its arithmetic mean. For example:

- 2000 + 1000: `μ = 1500`, `σ = 500`, so `R^eff = 1500 + 125 = 1625`.
- 1500 + 1500: `μ = 1500`, `σ = 0`, so `R^eff = 1500`.

This is deliberate. A team with one strong player and several weaker players is harder to beat than the raw average suggests, because the strong player can carry. Treating that team as stronger has three effects:

1. its expected win probability is higher;
2. a win therefore earns fewer points;
3. a loss therefore costs more points.

That is the anti-stacking mechanism. Homogeneous teams are not artificially inflated, while heterogeneous teams pay a “carry tax”.

The cap at 200 prevents a single extreme rating from making the effective rating nonsensical.

---

## 4. Win probability

The usual Elo logistic curve is used:

```text
E_A = 1 / (1 + 10^((R_B^eff - R_A^eff) / 400))
E_B = 1 - E_A
```

Because the two teams are complements, `E_A + E_B = 1`. This is the key property that makes the team exchange zero-sum.

The probabilities are clamped to `[1e-6, 1 - 1e-6]`:

- this guarantees a winner always has a strictly positive gain;
- it avoids floating-point underflow or a division by zero in extreme rating gaps;
- it has no practical effect for normal club ratings.

For a 400-point difference, the stronger team has about 90.9% expected score. For equal ratings, `E = 0.5`.

---

## 5. Match stakes

### 5.1 Base K and team size

The match-level K is:

```text
K_match = K_BASE × sqrt(n_A × n_B) × PlacementFactor
```

Why the geometric mean of team sizes?

- For equal teams `n_A = n_B = n`, `K_match = 200 × n`. The team pool for an even game is then `100 × n`, so each player moves by about 100 points.
- For a 1v1, `K_match = 200`, and a typical even game exchanges 100 points.
- For a 2v2, `K_match = 400`, the team pool is 200, and each player moves by about 100.
- For a 1v6, `K_match = 200 × sqrt(6) ≈ 489.9`. The solo player has more individual responsibility and therefore a larger stake; each player on the six-player team has a smaller share.
- For 2v3, `K_match = 200 × sqrt(6) ≈ 489.9`.

This keeps individual stakes roughly comparable across team sizes while preserving zero-sum at the team level.

### 5.2 Placement factor

New players need faster rating convergence. The placement factor is:

```text
PlacementFactor = 1 + 0.5 × exp(-average_games / 5)
```

where `average_games` is the average `games_played` of all participants before the match.

| Average games | PlacementFactor |
|---:|---:|
| 0 | 1.500 |
| 5 | 1.184 |
| 10 | 1.067 |
| 20 | 1.009 |
| 30 | 1.001 |

A match involving entirely new players has 50% larger stakes. A match between established players is effectively at the base K. Because the same factor multiplies both teams, the core exchange remains zero-sum.

### 5.3 Margin-of-victory multiplier

The margin uses the loser’s score, exactly as the sport suggests:

```text
d = 13 - loser_score
M(d) = clamp( (d / 5)^0.7, 0.5, 1.9 )
```

Examples:

| Score | `d` | `M` |
|---|---:|---:|
| 13–12 | 1 | 0.500 |
| 13–11 | 2 | 0.527 |
| 13–10 | 3 | 0.699 |
| 13–9 | 4 | 0.855 |
| 13–8 | 5 | 1.000 |
| 13–7 | 6 | 1.136 |
| 13–4 | 9 | 1.509 |
| 13–0 | 13 | 1.900 |

The exponent `0.7` makes the curve concave:

- from 13–8 to 13–0, the margin increases by 2.6× but the multiplier only rises to 1.9×;
- from 13–8 to 13–12, the multiplier drops to the floor of 0.5;
- a 13–12 win still gives points, but a 13–0 win is almost twice as valuable.

The floor is important: without it, a very close match could produce a near-zero or even negative winner delta after rounding. The cap prevents double-digit blowouts from inflating the ladder.

### 5.4 Unfinished matches

The multiplier depends only on the **loser’s score**, not on whether the winner reached 13.

Thus:

- an unfinished 10–7 and a finished 13–7 both use `d = 6` and `M = 1.136`;
- an unfinished 10–2 and a finished 13–2 both use `d = 11` and `M = 1.737`.

This is the key anti-abandonment design. If a losing team leaves at 7 points, the recorded result is still treated with the same margin as if the game had finished 13–7. Leaving does not reduce the margin multiplier. If the losing team keeps playing and scores more points, `d` decreases and their loss decreases. The incentive is therefore to continue playing.

### 5.5 Completion bonus

When the winner reaches 13 points, every participant receives:

```text
+1 Elo
```

This is applied after the team pool is distributed. It makes finishing strictly better than leaving for both teams, because the base stakes for an unfinished game at the same loser score are identical.

The completion bonus is intentionally small. It is the only non-zero-sum component of the system; see the inflation audit below.

---

## 6. Zero-sum team pools

For team A:

```text
P_A = K_match × (S_A - E_A) × M
```

For team B:

```text
P_B = K_match × (S_B - E_B) × M
```

Since:

```text
S_A + S_B = 1
E_A + E_B = 1
```

we get:

```text
P_A + P_B = K_match × M × (1 - 1) = 0
```

The match exchange is therefore exactly zero-sum before the completion bonus, rounding, sign clamps, and the database rating floor.

If team A wins:

```text
P_A = +K_match × (1 - E_A) × M
P_B = -K_match × E_B × M
```

If team A loses, the signs reverse.

---

## 7. Individual distribution

A team pool is not split equally. The player who was most expected to win receives less of a win and more of a loss. That is how the system handles a star carrying weaker teammates.

### 7.1 Individual expected score

For player `i` in team A:

```text
E_i = 1 / (1 + 10^((R_B^eff - R_i) / 400))
```

`R_B^eff` is the effective rating of the opposing team.

- A high-rated player has a high `E_i`.
- A low-rated player has a low `E_i`.

### 7.2 Contribution weights

For a winning team:

```text
u_i = 1 - E_i
```

The high-rated player has a small `u_i`; the underdog teammate has a large `u_i`.

For a losing team:

```text
u_i = E_i
```

The high-rated player has a large `u_i` and therefore absorbs more of the loss.

Normalize:

```text
v_i = u_i / Σ u_j
```

Then blend with an equal share to avoid extreme weights:

```text
w_i = (1 - 0.65) × (1 / n) + 0.65 × v_i
```

The 35% equal-share component guarantees that a star still receives some credit for a win and that a carried player does not receive all of it.

The final base delta is:

```text
Δ_i^base = P_team × w_i
```

Because `Σ w_i = 1`, the team distribution preserves `P_team` before rounding.

### 7.3 Rounding

Ratings are stored as integers. The implementation:

1. takes the floor of each raw delta;
2. distributes the remaining integer units to the players with the largest fractional parts (largest-remainder method);
3. adds the completion bonus if the game finished;
4. clamps winners to `>= 0` and losers to `<= -1`.

The rounding step preserves the sum of each team’s pool exactly before the completion bonus.

### 7.4 Sign guarantees

- A winner’s base pool is always positive, because `E_A < 1`, `M >= 0.5`, and all weights are positive.
- The largest-remainder rounding does not turn a positive raw delta into a negative integer.
- Winners are finally clamped with `max(0)`.
- Therefore a winner never loses rating points.
- A loser’s pool is always negative, and the final clamp is `min(-1)`, so a recorded loss always costs at least one point.

The `min(-1)` rule means a losing player is never rewarded by the completion bonus alone, even in an extreme mismatch.

---

## 8. Full algorithm

```text
inputs:
    left_score, right_score
    left_team:  [{id, elo, games_played}, ...]
    right_team: [{id, elo, games_played}, ...]

left_won       = left_score > right_score
winner_score   = max(left_score, right_score)
loser_score    = min(left_score, right_score)
finished       = winner_score == 13

M              = clamp((13 - loser_score) / 5)^0.7, 0.5, 1.9)
R_left_eff     = mean(left) + min(200, 0.25 * stddev(left))
R_right_eff    = mean(right) + min(200, 0.25 * stddev(right))
E_left         = 1 / (1 + 10^((R_right_eff - R_left_eff) / 400))
E_right        = 1 - E_left
placement      = 1 + 0.5 * exp(-average_games / 5)
K_match        = 200 * sqrt(len(left) * len(right)) * placement

P_left  = K_match * (S_left  - E_left)  * M
P_right = K_match * (S_right - E_right) * M

for each team:
    compute individual E_i against the opposing effective rating
    u_i = (1 - E_i) if won else E_i
    v_i = u_i / sum(u)
    w_i = 0.35 / n + 0.65 * v_i
    raw_delta_i = P_team * w_i
    round raw deltas to integers preserving the team sum
    if finished: add +1 to every player
    if won: delta_i = max(delta_i, 0)
    else:   delta_i = min(delta_i, -1)
```

The whole algorithm is `O(n_A + n_B)`, with at most 12 players.

---

## 9. Balance audit

### 9.1 Is the system zero-sum?

**Core match exchange: yes.**

```text
P_A + P_B = 0
```

The same `K_match` is used for both teams, and the logistic expectations are complementary.

**Individual redistribution: yes.**

Each team’s pool is distributed among its own players with normalized weights. The largest-remainder rounding preserves the team sum before the completion bonus.

**Completion bonus: no, deliberately.**

Every finished game adds `+1` per participant to the total rating pool:

- 1v1: `+2` total;
- 2v2: `+4` total;
- 6v6: `+12` total.

This is the only intentional inflation source. It is bounded, linear in finished games, and can be disabled by setting `COMPLETION_BONUS = 0` in `src/server/elo.rs`. For a strict zero-sum league, disable it. If it is kept, a seasonal soft reset or global normalization can be used to keep absolute ratings from drifting too far over many seasons.

**Database floor: a small legacy leak.**

The schema requires `users.elo >= 0`, and updates use `GREATEST(0, elo + Δ)`. If a player near 0 loses more points than they have, the missing points are not paid to the winner. This is a safety floor, not a mathematical property of the Elo formula. With the application’s default rating of 400 and typical losses below 300, this is rare. For strict conservation, the floor should be removed or a carry-over debt should be implemented.

### 9.2 Proof that a winner never loses points

For a winner:

```text
E_A < 1
M >= 0.5 > 0
K_match > 0
w_i > 0
```

Therefore:

```text
P_A = K_match × (1 - E_A) × M > 0
Δ_i^base = P_A × w_i > 0
```

After rounding, the delta is non-negative; after the final clamp it is `>= 0`. With a finished game, the `+1` bonus makes it at least `+1`.

This holds even for a 13–12 win against a much lower-rated opponent:

- Example D below: the 1700 favourite gains only `+10`, but still gains;
- an even 1500 vs 1500 game at 13–12 gives about `+51`;
- an extreme 2500 vs 400 at 13–12 gives `+1`.

### 9.3 Close wins, blowouts, and leaving early

- The `M` floor of `0.5` guarantees that a close win is still worth something.
- The `M` cap of `1.9` prevents a fanny from producing absurd swings.
- The loser-score-based margin means that an unfinished 10–7 and a finished 13–7 are rated identically before the completion bonus.
- Finishing therefore gives `+1` to every player instead of leaving at the same loser score.
- If the losing team keeps playing and scores more points, `d` decreases and their eventual loss decreases. This is the correct incentive: play on, score points, reduce the margin.

### 9.4 New players and smurfing

New-player handling:

- placement factor up to `1.5×` for a match whose participants average 0 games;
- decay to about `1.067×` at 10 average games and `1.009×` at 20;
- after roughly 30–50 games, the factor is effectively 1.

This accelerates convergence for legitimate new players.

Anti-smurfing measures:

1. **Membership-gated account creation.** New accounts are created through the existing Helloasso adherent flow. Creating many accounts requires multiple memberships.
2. **Fast convergence.** A genuinely strong new account moves quickly to its true rating instead of remaining in low-rated games for a long time. Dynamic K shortens the smurfing window; it does not reward smurfs with a permanent low rating.
3. **Dispersion penalty.** A strong player teaming with low-rated accounts is treated as a stronger team, reducing the points won from such stacking.
4. **Individual weighting.** The strong player receives a smaller share of a win, so the “star + low-rated accounts” farming pattern is less profitable.
5. **Audit and moderation.** The admin audit log and account inspector make suspicious accounts and match patterns visible.

No pure Elo system can fully prevent smurfing. The correct practical strategy is identity gating, fast convergence, and moderation, all of which are present.

### 9.5 Newbie versus professional

If a 400-rated newbie loses to a 2500-rated professional:

```text
E_newbie ≈ 1e-6 (clamped)
M <= 1.9
K_match = 200 for 1v1, up to 300 for a newbie placement match
base loss ≈ -0.0004
```

After the completion bonus and the `min(-1)` clamp, the newbie loses `1` point. This is not over-penalizing a beginner; it reflects that the result was overwhelmingly expected.

Conversely, if the newbie wins, the gain is large but still bounded by the expectation clamp, the MoV cap, and the placement factor.

### 9.6 Team size asymmetry

The geometric mean of team sizes keeps the system sensible for 1v6, 2v3, etc.

Example: equal 1500 ratings, 1v6, finished 13–8:

```text
K_match = 200 × sqrt(6) ≈ 489.9
P_solo  = 489.9 × 0.5 ≈ 244.9
```

- the solo player changes by about 245 points before the completion bonus;
- each of the six opponents changes by about 41 points;
- the total remains zero-sum before bonus.

This matches the responsibility asymmetry: the solo player owns the entire result, while each member of the large team owns a smaller share.

### 9.7 Rating gaps and numerical safety

- `E` is clamped to avoid an exact 0 or 1.
- `M` is clamped to `[0.5, 1.9]`.
- `DISPERSION_CAP` prevents extreme effective ratings.
- The largest-remainder rounding avoids accumulated rounding drift.
- Winners are clamped to non-negative deltas.
- Losers are clamped to at most `-1`.
- The implementation uses `f64` internally and stores `i64` deltas.

---

## 10. Numerical scenarios

All scenarios use established players with about 50 games, so the placement factor is effectively `1.000`. The completion bonus of `+1` is included where the winner reached 13.

### Case A — Even match, 13–8

Ratings: 1500 vs 1500, 1v1.

```text
E = 0.5
M = (5/5)^0.7 = 1
K_match = 200
P = 200 × (1 - 0.5) × 1 = 100
```

Base exchange: `+100 / -100`.

With completion bonus: `+101 / -99`.

This satisfies the ~100-point target.

### Case B — Even match, fanny 13–0

Ratings: 1500 vs 1500, 1v1.

```text
d = 13
M = clamp((13/5)^0.7, 0.5, 1.9) = 1.9
P = 200 × 0.5 × 1.9 = 190
```

Base exchange: `+190 / -190`.

With completion bonus: `+191 / -189`.

A fanny is worth about 1.9× a typical win, not 2.6× linearly.

### Case C — Huge upset, 1300 beats 1700, 13–4

```text
E_underdog = 1 / (1 + 10^((1700 - 1300)/400)) = 1/11 ≈ 0.0909
d = 9
M = (9/5)^0.7 ≈ 1.509
P = 200 × (1 - 0.0909) × 1.509 ≈ 274.4
```

With completion bonus: `+275 / -273`.

The underdog is rewarded heavily, while the favourite’s loss is large because the result was very unexpected.

### Case D — Expected win, 1700 squeaks past 1300, 13–12

```text
E_favourite = 10/11 ≈ 0.9091
d = 1
M = 0.5
P = 200 × (1 - 0.9091) × 0.5 ≈ 9.09
```

With completion bonus: `+10 / -8`.

The favourite gains very little, and the underdog loses very little. A close loss does not destroy the underdog, but it still costs points.

### Case E — Even 2v2, 13–8

Ratings: 1500 + 1500 vs 1500 + 1500.

```text
K_match = 200 × sqrt(2 × 2) = 400
P = 400 × 0.5 × 1 = 200
```

Before bonus, each winner gets `+100` and each loser gets `-100`.

With completion bonus: each winner `+101`, each loser `-99`.

This is the team-size calibration: equal teams of any size give each player roughly 100 points.

### Case F — Star + weak teammate, 13–8

Team A: 2000 + 1000. Team B: 1500 + 1500.

```text
μ_A = 1500
σ_A = 500
R_A^eff = 1500 + 0.25 × 500 = 1625
R_B^eff = 1500

E_A = 1 / (1 + 10^((1500 - 1625)/400)) ≈ 0.6725
K_match = 400
P_A ≈ 400 × (1 - 0.6725) × 1 = 131
```

Individual weights against `R_B^eff = 1500`:

- star (2000): `E_i ≈ 0.947`, contribution `1 - E_i ≈ 0.053`;
- low-rated teammate (1000): `E_i ≈ 0.053`, contribution `≈ 0.947`.

After normalizing and blending with the equal share:

- star receives about 21% of the pool;
- teammate receives about 79%.

Final rounded base deltas are approximately:

```text
star:      +27
teammate:  +103
```

With completion bonus:

```text
star:      +28
teammate:  +105
```

If the same team loses 7–13:

```text
M ≈ 1.136
P_A ≈ -305.6
star:      about -241
teammate:  about -63
```

The star gains less when carried and loses more when the team loses. This is the intended carry adjustment.

### Case G — New players, 13–8

Ratings: 1500 vs 1500, both with 0 games.

```text
PlacementFactor = 1 + 0.5 × exp(0) = 1.5
K_match = 200 × 1.5 = 300
P = 300 × 0.5 × 1 = 150
```

With completion bonus: `+151 / -149`.

New-player matches have larger stakes so ratings converge faster.

### Case H — Unfinished 10–7 versus finished 13–7

Both use `loser_score = 7`, so:

```text
d = 6
M = (6/5)^0.7 ≈ 1.136
```

For an even 1v1:

```text
P = 200 × 0.5 × 1.136 ≈ 113.6
```

Unfinished 10–7 (no completion bonus): about `+114 / -114`.

Finished 13–7 (completion bonus): about `+115 / -113`.

The margin is identical; finishing is strictly better for both players. This is the anti-abandonment incentive.

---

## 11. Implementation in Leaderboule

### Files

- `src/server/elo.rs`
  - full rating engine;
  - all constants;
  - `compute_changes`;
  - MoV, team rating, individual distribution, rounding;
  - 12 unit tests.
- `src/server/db.rs`
  - `UserWithElo` now carries `games_played`;
  - `map_usernames` loads the players’ current Elo and experience;
  - `register_game` persists `game_players.elo_change` and updates users atomically.
- `src/server/moderation.rs`
  - game edits reverse the old `game_players.elo_change` values first;
  - then reload the post-reversal Elo and `games_played`;
  - then recompute the new game with the same engine;
  - the whole operation remains transactional and audited.

### Data flow

1. The client submits only usernames, team sides, and scores.
2. The server validates the score and team structure with `verify_game`.
3. The server loads the authoritative ratings and experience from PostgreSQL.
4. `compute_changes` returns one integer delta per participant.
5. The game, participations, and user updates are written in one transaction.
6. The audit log records moderation edits.

The client cannot manipulate its own rating or the ratings of other players.

### Tests

The unit tests cover:

- the ~100-point typical 1v1 case;
- the fanny multiplier;
- a huge upset;
- an expected favourite’s narrow win;
- equal 2v2 and 3v3 distribution;
- the star/carried-teammate weighting;
- the winner-never-loses guarantee;
- unfinished games using the loser score;
- the zero-sum unfinished exchange;
- MoV monotonicity and bounds;
- placement acceleration.

Run them with:

```bash
cargo test --no-default-features --features server
```

---

## 12. Future extensions

The current system is intentionally a single-number Elo system. Two useful upgrades would be:

1. **Glicko-2 / rating deviation**
   - Each player would carry a rating deviation.
   - New players and inactive players would have higher uncertainty.
   - The displayed rating would remain an integer, but the K factor would derive from uncertainty instead of only games played.

2. **Seasonal soft reset**
   - Compress ratings toward the mean at the start of a season.
   - This also offsets the small completion-bonus inflation if strict long-term stability is desired.

3. **Inactivity decay**
   - Gently reduce confidence or rating for players who have not played for a long time.
   - This should be applied carefully; it is not part of the current implementation.

The current system is deliberately transparent: every player can understand why a 13–8 win is worth about 100 points, why a 13–0 is worth more, and why a 13–12 is worth less.
