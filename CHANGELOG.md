# Changelog

All notable changes to rsshogi will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.2.0] - 2026-08-15

### Changed

- Slider attacks, move generation, mate-in-one detection, position serialization, primitive
  types, and external book handling were independently reimplemented. The supported package
  remains MIT-licensed, and the core crate now includes its own `LICENSE` file.
- Legal-move APIs return legal moves. Callers define their preferred order by sorting or
  scoring the result; `LegalAll` provides the complete optional-nonpromotion set.
- `AperyMove` and `AperyMove32` remain nominal transparent newtypes with their own raw
  layouts. `to_move` and `to_apery` convert between them and `Move` / `Move32`.

### Fixed

- Packed position, PACK, YBB, and DB2016 readers validate inventories, canonical move
  encodings, and game outcomes while reporting malformed input as errors.
- Mate-in-one detection checks the moving side's king safety before returning a checking move.
- Wheel builds exclude in-place extension artifacts left by `maturin develop`, allowing a
  tested working tree to produce a clean release artifact.

### Compatibility

- HCP, PackedSfen, PACK, HCPE, YBB, SBK, `Position::key`, and the low 64 bits of the
  `hash-128` key retain their established byte representation. Existing data remains usable
  with 1.2.0.
- PACK plies continue to use the Apery 16-bit layout: the source field values 81 through 87
  encode drops and bit 14 encodes promotion. Drops, promotions, and non-starting positions
  round-trip through the Rust and Python record APIs. Converting a `Record` to PACK clamps
  evaluation values to the centipawn range `-32000..=32000`.
- The generic move generators and their `Move32` facades return the expected move sets,
  including quiet checking drops, optional unpromoted variants, and mode-specific behavior.
  `LegalAll` is the complete legal set; `Legal` intentionally omits selected optional
  unpromoted moves but includes the legal unpromoted lance move to the third rank.
- DB2016 files are read incrementally. A position group containing a malformed move line is
  reported by `iter_entries()` without discarding later groups, `resign` remains a valid move
  token, and omitted `count` and ply fields remain omitted when written unless an option
  supplies them.
- `Move::from_usi` and `Move32::from_usi` accept USI `0000` as the null move. `Eval` ordering
  follows the underlying signed numeric value, promoted pieces are not hand-piece values,
  and `Hand::add` / `Hand::sub` panic on overflow or underflow before updating the packed value.
- KI2 notation retains the full-width space after `同`, and ambiguity resolution includes
  geometric origin candidates even when a candidate piece is pinned.

### Removed

- **Breaking: the peta_shock-compatible book solver has been removed.** This removes
  `solve_peta_shock_book`, `PetaShockOptions`, and the peta_shock-only lossy profile from
  `YaneuraOuDb2016WriteOptions`. The lossless DB2016 reader and writer remain available.
- **Breaking: generated Qugiy mask constants are no longer public.**
  `QUGIY_STEP_ATTACKS`, `QUGIY_ROOK_MASK`, and `QUGIY_BISHOP_MASK` were implementation
  details; use `rook_attacks`, `bishop_attacks`, and the public beam tables instead.
- **Breaking: `board::Bitboard256` has been removed.** It was an implementation-detail
  packed slider helper. Use the public `Bitboard` type and attack functions.

## [1.1.1] - 2026-08-11

### Changed

- `EngineInfo.eval` is now documented as a centipawn value from the side-to-move
  perspective before its record entry. KIF `**評価値=` input and output convert this
  internal value to and from ShogiHome-compatible Black perspective.

## [1.1.0] - 2026-08-05

Despite the minor version bump, this release carries source-incompatible API changes: the
Zobrist key redesign, the CSA move formatting change, and the SAZ2 self-play format moving
to version 2. It also changes three persisted formats — `Position::key` values, the static
book binary, and SAZ2 self-play chunks — so stored data produced by 1.0.x must be
regenerated. Read *Compatibility* before upgrading.

The CSA implementation was audited against the official V2.2 and V3.0 specifications.
V2.2 was already covered feature for feature, but the audit found bugs in how it was
read; those and the V3.0 gaps are listed under *Fixed*.

### Changed

- **`Move32::to_csa` now emits the leading side-to-move sign**, so `7g7f` formats as
  `+7776FU` rather than `7776FU`. The sign is a mandatory part of a CSA move record, so the
  previous output was not valid CSA on its own. The sign is taken from the colour of the
  move's own piece, which is why the method now returns `None` for a partial `Move32` that
  carries no piece information — including a partial drop, which previously produced a
  colourless body. A drop that carries its colour, such as one built with `Move32::drop` or
  taken from a position, still formats normally. This also applies to `Move32.to_csa()` in
  the Python bindings.
- **Breaking: `Position::key` values have changed for any position where either side holds
  a piece in hand.** The hand-piece contribution is now composed with XOR over a
  count-indexed table instead of `base * count` with add/sub. Any persisted `key` must be
  regenerated; see *Compatibility* below.
- `ZobristKey` is now a pure XOR monoid. `add`, `sub`, `mul_u64`, and `xor` were removed;
  use the `BitXor` / `BitXorAssign` operators. The remaining surface is `new`, `from_u64`,
  `low_u64`, `high_u64`, `From<u64>`, and the derived traits.
- `Zobrist::hand` now takes the piece count as `u32` and returns the key for holding
  exactly that many pieces, rather than a base value scaled by the count. Compose with XOR.
- `ZobristTable::hand_at_index` is now indexed by a flat `(piece type, count)` slot rather
  than by piece type. The slot layout is an implementation detail derived from `Hand`'s bit
  widths; use `Zobrist::hand` as the stable entry point.
- `Position::key` and `Position::board_key` are no longer `const fn`. The keys now live in
  the current state rather than being mirrored on `Position`.
- `PartialKeys::new` takes one fewer argument, and `MoveApplyFacts::from_delta` no longer
  takes a `hand_key_after`, following the removal of the material and hand keys below.
- **Breaking: the SAZ2 self-play format is now version 2**, carrying the network's raw
  outputs alongside the search results (see *Added*). A version 1 chunk is refused with
  `UnsupportedVersion` rather than being reinterpreted under the new layout, and 1.0.x
  cannot read what 1.1.0 writes. There is no migration path; see *Compatibility*.
- `SazSelfplayPosition` and `SazSelfplayPolicyEntry` gained fields, so struct literals no
  longer compile. In Python, `SazPosition(...)` and `SazPolicyEntry(...)` gained required
  arguments.
- `raw_prior` is validated as a distribution in its own right: it must sum to exactly
  65535 independently of `prior`, so a payload where only one of the two is well-formed is
  refused with `InvalidDistributionSum`.
- **The static book binary is now version 2.** Its structure is unchanged, but it stores
  `Position::key` values, whose meaning changed above. A version 1 file is now refused
  instead of loading and silently failing to match positions with pieces in hand.
- **The CSA parser now checks the side-to-move sign on a move line.** A line whose sign
  disagrees with the side to move — `-7776FU` when Black is to move — was previously
  accepted and silently rewritten on export; it is now rejected with the new
  `CsaError::MoveSideMismatch`, which carries the offending line so the producer can be
  identified. The move itself may well be legal, which is why this is not `IllegalMove`.
- CSA export no longer writes a fabricated `T0` for a move with no recorded elapsed time.
  The spec makes the elapsed-time line optional, and `T0` claimed the move took zero
  seconds. Records that do carry a time are unaffected.
- CSA export now writes back the terminal marker the record was parsed from, whenever the
  record carries one, so `%+ILLEGAL_ACTION` and `%-ILLEGAL_ACTION` survive a round trip
  instead of degrading to `%ILLEGAL_MOVE`. Note that this takes precedence over the
  terminal's kind, so rewriting the kind on a parsed record does not change the marker
  written. For a terminal with no recorded marker, `SpecialMove::WinByDefault`,
  `LoseByDefault`, and `Try` — none of which have a CSA equivalent — now map to `%CHUDAN`
  rather than `%KACHI`, which claimed a declaration win that never happened and, for
  `LoseByDefault`, awarded the game to the side that lost it.

### Added

- `parse_csa_games` and `parse_csa_games_bytes` return every game in a `/`-separated CSA
  file. `parse_csa_str` still returns the first game, and now stops cleanly at a separator
  instead of failing with `InvalidLine` when the game has no terminal marker. In Python:
  `Record.from_csa_games_str` and `Record.from_csa_games_file`.
- `export_csa_with_options` and `ExportOptions::with_csa_version` select the CSA output
  version. `CsaVersion::V3_0` writes a `V3.0` header preceded by a `'CSA encoding=...`
  declaration matching the output encoding, and emits millisecond elapsed times. The
  default stays `CsaVersion::V2_2`. In Python: `Record.to_csa(version="3.0")` and
  `Record.write_csa(..., version=…)`; a V3.0 write refuses an encoding that CSA cannot
  declare (anything other than UTF-8 or Shift_JIS) rather than declaring a false one.
- `Zobrist::hand_delta(color, piece_type, from, to)` returns the key difference for a hand
  count changing from `from` to `to`, equal to the XOR of both counts' keys.
- `Position::key_after`, `Position::board_key_after`, `Position::board_key_after_move`, and
  `Position::key_after_null` compute the key a move would produce without mutating the
  position. These are intended for transposition-table prefetch in a search engine.
  `key_after_move` is no longer gated behind the `book` feature.
- `Hand::count_mask` exposes the bit mask of a `HandPiece`'s count field.
- `ZobristTable::HAND_SLOT_COUNT` gives the length of the hand table's flat slot dimension.
- **SAZ2 self-play records now store the network's raw outputs**, which version 1 kept
  nowhere. Every row carries all four, with no per-row opt-out, so a decoder rejects a
  missing value structurally rather than leaving it for the consumer to notice:
  - `SazSelfplayPolicyEntry::raw_prior` — the prior before Dirichlet noise and
    proven-edge suppression. `prior` remains the value after both.
  - `SazSelfplayPosition::raw_wdl` — the WDL straight from the network, without search
    aggregation. `root_wdl` remains the aggregated value.
  - `SazSelfplayPosition::raw_mate` — the mate head's probability, as `u16 / 65535`. This
    is a different quantity from the optional `mate`, which is a search-proven result.
  - `SazSelfplayPosition::raw_moves_left` — the moves-left head's prediction, in plies
    fixed-point at `plies * 32`. This is a different quantity from `plies_left`, which
    comes from the game outcome.

  The policy is stored as probabilities over the legal moves rather than as 1496-dimension
  logits: the masked distribution's entropy is exact from the probabilities, and a softmax
  temperature can be re-derived without them. Every value is integer fixed-point, so the
  format cannot represent a NaN. All four are exposed in Python as read-only properties.

### Removed

- `Position::hand_key`. The composite key is maintained directly, so the hand-only key no
  longer exists as an intermediate quantity. Consumers that need hand identity should
  compare the raw `Hand` values, as the superior-position check already does.
- `MoveApplyFacts::hand_key_after`, for the same reason. `board_key_after` and `key_after`
  remain.
- `Zobrist::material` and `PartialKeys::material`. The material key had no consumer inside
  or outside the crate while costing an update on every move. `PartialKeys::material_value`
  (the `i32` material evaluation) is a different quantity and is retained.

### Fixed

- **A CSA record whose game ends before the first move lost its terminal entirely.** The
  header scanner treated a line starting with `%` as the side-to-move token unless it was
  at least seven characters long, so the six-character markers `%TORYO`, `%TSUMI`,
  `%KACHI`, and `%ERROR` were swallowed and the record came back with `GameResult::Invalid`
  and no terminal node. Longer markers such as `%FUZUMI` were unaffected, which is why this
  went unnoticed.
- **Millisecond elapsed times (`T15.123`, added in CSA V3.0) were dropped, not rounded.**
  The parser rejected the fractional token and discarded the whole `T` line, so every
  elapsed time in a V3.0 record was lost. Values are now parsed to the millisecond, with a
  fraction of at most three digits as the spec requires.
- **The `'** <eval> <pv> #<nodes>` analysis line (CSA V3.0) was silently discarded**, and
  CSA export never wrote an evaluation at all — even one parsed from the non-standard
  `'**評価値=` form. The evaluation and node count now populate `EngineInfo`, the principal
  variation is kept verbatim in `EngineInfo::extras["csa_pv"]` (it may contain `+PASS` or
  `%TORYO`, which no move type can represent), and export reconstructs the line. An
  analysis line whose evaluation cannot be read — including one outside the `i16` range
  `Eval` can hold, and a non-numeric `'**評価値=` — is preserved verbatim in
  `extras["csa_analysis_raw"]` rather than being dropped. Several such unreadable lines on
  one move accumulate instead of overwriting each other; the spec places one `'**` per
  move, so among readable lines the last one wins. An analysis line following the terminal
  marker is kept on the terminal. One placed before the first move is skipped: the
  specification gives `'**` no placement at the initial position — §2.8(1) allows a `'*`
  comment there but §2.8(2) states no placement at all, and §2.8(3) defines the principal
  variation as the continuation of the preceding move. A `'*` comment at the initial
  position is unaffected and is still kept in `Record::initial_comment`.
- **CSA terminal markers awarded the game to the wrong side.** `%ILLEGAL_MOVE` and
  `%TIME_UP` are both a loss for the side to move, so the *opponent* wins; both were
  resolved the other way round, handing the game to the offender and to the player who ran
  out of time. `%+ILLEGAL_ACTION` (White wins) and `%-ILLEGAL_ACTION` (Black wins) are
  fixed by the marker itself, but were also being resolved from the side to move, so they
  inverted on roughly half of all records. `%ILLEGAL_ACTION` additionally resolved to
  `SpecialMove::WinByIllegalMove` or `LoseByIllegalMove` depending on the side to move; it
  is now always `WinByIllegalMove`, matching the winner-relative naming the other markers
  use.
- The V3.0 per-side time-control keys are now accepted with either spelling. The spec's
  prose uses `$TIMET+:` / `$TIMET-:` while its own examples use `$TIME+:` / `$TIME-:`; only
  the latter was recognised, so the former fell through to generic attributes. Output
  normalises to the example spelling.
- The `board::zobrist` module documentation claimed the hash values were compatible with
  YaneuraOu. They never were: YaneuraOu fixes `side = 1` and masks every key with `& ~1ULL`
  to encode the side to move in bit 0, which rsshogi does not do. Only the table generation
  scheme is shared.

### Compatibility

- **CSA export output changed in several ways beyond the move sign**, all of them fidelity
  fixes, but enough that a byte-for-byte comparison against 1.0.2 will differ for records
  that hit any of them: a move with no recorded elapsed time no longer emits `T0`; a
  terminal parsed from `%+ILLEGAL_ACTION` / `%-ILLEGAL_ACTION` round-trips instead of being
  written as `%ILLEGAL_MOVE`; a record carrying an evaluation now emits a `'** <eval>` line
  where before none was written at all; `$TIMET+:` / `$TIMET-:` input is normalised to
  `$TIME+:` / `$TIME-:`; and `WinByDefault` / `LoseByDefault` / `Try` terminals write
  `%CHUDAN` instead of `%KACHI`. `ExportOptions` also gained a field, so struct literals do
  not compile; use `ExportOptions::new(encoding)` with `with_csa_version`.
- **CSA terminal markers now assign the winner as the specification states**, which
  reverses the result on affected records. Any pipeline that derived a result from
  `%ILLEGAL_MOVE`, `%TIME_UP`, or `%±ILLEGAL_ACTION` — including conversion to HCPE, pack,
  or SAZ2 — produced the wrong winner and must be rerun. Code that matched on
  `SpecialMove::LoseByIllegalMove` for an `%ILLEGAL_ACTION` terminal will no longer match;
  see *Fixed*.
- **A CSA move line whose sign disagrees with the side to move is now rejected**, with
  `CsaError::MoveSideMismatch`. Such a record parsed successfully before, with the sign
  discarded and re-derived on export. If a producer in your pipeline emitted the wrong
  sign, its files stop parsing rather than round-tripping to something different from the
  input. `CsaError` gained a variant, so an exhaustive `match` on it no longer compiles.
- V3.0-only metadata keys (`$MAX_MOVES`, `$JISHOGI`, `$NOTE`, `$TIME+`) are still written
  under a `V2.2` header. Dropping them to reach strict V2.2 conformance would lose data,
  and readers skip unrecognised `$` keys. Select `CsaVersion::V3_0` for a conformant file.
- **Callers that prepended their own `+` / `-` to `Move32::to_csa` must drop it**, or the
  sign will be doubled. Callers that fed the result to `Position::move_from_csa` need no
  change: it already accepted both the signed and the unsigned form. **The sign change on
  its own does not alter any record or board output**: the record writer takes the sign from the move
  instead of from the position, which is the same value, and board output (`board_to_csa`,
  `Board.to_csa()`) does not go through `Move32::to_csa` at all. Record output still
  differs from 1.0.2 for the unrelated reasons listed in the first bullet above.
- **Persisted `Position::key` values must be regenerated.** This includes book files, since
  `BookKey` is a `ZobristKey` produced from `key`. Positions and moves are unaffected; only
  the key values change, so a book can be rebuilt from its existing position and move data
  — rerun whatever pipeline produced it. There is no in-place key migration, because the
  mapping from an old key to a new one is not computable from the key alone. The static
  book binary's version was raised to 2 in the same release, so a 1.0.x file fails to load
  with `Unsupported("version mismatch")` instead of loading and quietly missing every
  position with a piece in hand.
- **SAZ2 self-play data must be regenerated; there is no converter.** 1.1.0 refuses a
  version 1 chunk with `UnsupportedVersion`, and 1.0.x refuses what 1.1.0 writes. The raw
  network outputs added in version 2 are not recoverable from a version 1 chunk — they were
  never stored — so a conversion would have to fabricate them, which is exactly the silent
  corruption the version check exists to prevent. Reading existing version 1 archives
  requires pinning 1.0.x.
- **Rust code constructing `SazSelfplayPosition` or `SazSelfplayPolicyEntry` by struct
  literal will not compile**, since both gained fields. In Python, `SazPosition(...)` and
  `SazPolicyEntry(...)` gained required arguments, so existing calls fail with a `TypeError`
  for missing arguments rather than silently shifting values. The new arguments sit next to
  the quantities they mirror — `raw_prior` after `prior`, and `raw_wdl` / `raw_mate` /
  `raw_moves_left` after `outcome_wdl` — so keyword calls need only the new names added.
- **`board_key` and the partial keys are bit-identical to 1.0.2.** The hand table is drawn
  last from the generator PRNG, so `side`, `no_pawns`, and the piece-square table keep their
  previous values. `Position::board_key`, `PartialKeys::pawn`, `PartialKeys::minor`, and
  `PartialKeys::non_pawn` therefore did not change, and neither did `key` for a position
  where both sides have an empty hand. Do not rely on this to skip a book regeneration: a
  book that contains any position with a piece in hand is invalid.
- The Python bindings are unaffected by the Zobrist redesign at the API level.
  `zobrist_hash()` keeps its signature; only the value it returns changes. `hand_key`,
  `board_key`, and `material_key` were never exposed to Python. The CSA and SAZ2 changes
  above do reach Python: the SAZ2 constructors gained required arguments, `Move32.to_csa()`
  emits the sign and returns `None` for a partial move, `Record.result()` reverses on the
  affected terminal markers, `Record.from_csa_str()` rejects a move line whose sign
  disagrees with the side to move, and `Record.to_csa()` output changed as described above.
- A 64-bit build and a 128-bit (`hash-128`) build still agree on the low 64 bits of every
  key, as before.

## [1.0.2] - 2026-07-21

### Fixed

- The SAZ2 self-play codec no longer accepts a forged chunk whose policy `prior` values
  overflow the running total. The sum was accumulated in a `u32`, so a chunk with 65,539
  policy entries could wrap to exactly 65535 and pass validation in release builds, where
  overflow checks are disabled. The same input aborted debug builds with an overflow panic.
  Both the encoder and the decoder now validate the distribution without any possibility of
  overflow.

### Compatibility

- The SAZ2 wire format and the public API are unchanged. Any chunk that was valid before
  remains valid and encodes to the same bytes; only invalid input is affected.

## [1.0.1] - 2026-07-21

Distribution-only release. The library code, public API, and runtime behavior are
identical to 1.0.0.

### Added

- Python wheels for macOS x86_64 (Intel) in the standard `rsshogi` distribution.
- Python wheels for Linux aarch64 (manylinux) in the standard `rsshogi` distribution.

### Changed

- The Rust crate `rsshogi` is republished at 1.0.1 to keep the crate, the Python
  distributions, and the release tag on the same version. It contains no code changes.

### Compatibility

- The AVX2 distribution `rsshogi-avx2` keeps its x86_64-only platform set. AVX2 is an
  x86 instruction set, so there is no arm64 AVX2 build.

## [1.0.0] - 2026-07-19

### Added

- Rust crate `rsshogi` for board state, move generation, records, book formats, and training-data formats.
- Python distributions `rsshogi` and `rsshogi-avx2`. Both provide the `rsshogi` import package.
- Rust and Python documentation with runnable examples.

### Compatibility

- The standard and AVX2 Python distributions are mutually exclusive because both provide the same import package.

[Unreleased]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.0...HEAD
[1.2.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/nyoki-mtl/rsshogi/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.1.0
[1.0.2]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.2
[1.0.1]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.1
[1.0.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.0
