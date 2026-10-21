# Changelog

All notable changes to rsshogi will be documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [1.6.0] - 2026-10-21

`moved_piece_before` と `moved_piece_before_move`、通常の着手結果 `MoveApplyFacts`、
`clone_for_search`、反復判定、容量確保 API は引き続き提供する。
二歩の照会 `has_pawn_on_file` と利きの primitive `rook_rank_attacks` も維持する。

### Deprecated

- 探索用のキー予測、着手前メタデータ、探索スタック用の型、予約領域専用の着手、
  履歴保持上限の設定と、未使用の補助 API を非推奨にした。
  すべて 1.x 中は従来のシグネチャと動作を維持し、2.0.0 で削除予定とする。
  対象と移行方法は [1.6.0 の移行ガイド](docs/book/src/getting-started/migration-1.6.md) を参照する。

### Performance

- `Position::clone()` の履歴チャンクの複製方法を改善した。
  全履歴の独立した所有権と、事前確保済みの容量を維持する。
- `Position::clone_from()` が複製先の履歴領域を再利用する。
  全履歴と複製元の予約容量を維持し、複製先に余る領域は保持する。
  同じ保存先へ局面を繰り返し複製する場合に利用できる。
- `Position::clone_for_search()` が根履歴の board key 索引を準備する。
  一致候補がない場合だけ根以前の千日手走査を省き、一致候補がある場合は従来の詳細判定を行う。
  連続王手と持ち駒の優劣を含む判定の意味は変えない。
  索引構築と照会には費用があり、すべての局面で高速化するものではない。
- `rook_rank_attacks()` を専用実装から `rook_attacks()` と段マスクの合成に変更した。
  戻り値は維持するが、従来実装との速度の同等性は保証しない。

### Maintenance

- bindings 内部の未使用 getter 3 件と開発用依存 `shogi_core` を削除した。
- 計測、配置実験、外部データ照合用の example は source と再現手順を維持し、crates.io 配布から除外した。
  `sfen_roundtrip` は配布に含める。
- Python の配布物から `__pycache__` と `.pyc` を除外した。
- 探索連携の文書を `MoveApplyFacts` と `current_state_cache()` の実装に合わせて修正した。

## [1.5.1] - 2026-10-19

### Fixed

- `Position::is_pseudo_legal_move` と `Position::is_pseudo_legal_move32` は、
  `generate_all_legal_moves == false` でも行き所のない桂の不成を拒否する。
  先手の 1 段目と 2 段目、後手の 8 段目と 9 段目が対象。
  1.2.3 の修正は `true` の判定と駒打ちに限られており、`false` の判定には修正漏れがあった。
  `is_legal_move` / `is_legal_move32` と合法手生成は従来からこれらの手を拒否している。


## [1.5.0] - 2026-10-18

Native Book の API と保存形式に破壊的変更を含む。

### Added

- Native Book の新規構築でブロック局面数を 1..=16384 から指定できる。既定と推奨値は 64。Rust は `NativeBookBuilder::with_block_positions`、Python は `block_positions=` を使用する。保存値の取得と、compaction、別名保存、救出での維持にも対応する。

- Native Book v4 の reader / editor / builder を Rust と Python に追加。32B の局面キー、任意の評価値、コメント、文書メタデータ、ブロック単位の部分読込、差分保存、排他、保全付き復旧、別名救出、compaction に対応する。本体は可変長整数と既定 64 局面単位の Zstandard 圧縮、ブロックごとの CRC-32C と展開上限を使う。
- 文書単位の `depth_kind: u32` を追加。0 は未指定、1 は探索深さ、2 は定跡内の後退解析距離。未知の値も保存と再読込で保持する。
- DB2016、SBK、所有 IR から v4 への明示変換と欠落報告を追加。
- `Move::flip` / `Move::mirror_file` と `Move32::flip` / `Move32::mirror_file` を追加。
  盤面の 180 度回転（先後反転）と左右反転に対応する指し手を返す。`Move32::flip` は
  駒情報の手番も反転し、`mirror_file` は駒情報をそのまま保つ。`MOVE_NONE` や
  `MOVE_NULL` のような通常手でない値はそのまま返す。学習データの対称性拡張で使う。
- `Position::to_packed_sfen_flipped` / `Position::try_to_packed_sfen_flipped` を追加。
  SFEN 文字列を経由せずに先後反転局面の `PackedSfen` を得る。反転規則は
  `Position::to_sfen_flipped` と共有する。
- Python bindings に反転 API を追加。`Move.flip_side()` / `Move.mirror_file()`、
  `Move32.flip_side()` / `Move32.mirror_file()`、`Board.to_sfen_flipped()`、
  `Board.to_packed_sfen_flipped(out=None)` を公開する。Python 側の命名は既存の
  `Square.flip_side()` / `Bitboard.flip_side()` に揃えた（core の `flip` に対応する）。

### Breaking changes

- `BookError` に `Busy`、`NativeLog`、`SaveUncertain` を追加し、`#[non_exhaustive]` を付けた。利用側の Rust の `match` は新しい variant への対応に加え、将来の拡張を受ける `_` 分岐が必要になる。
- 旧 `Book`、`BookKey`、`BookMove`、`BookEntry`、`MemoryBook`、`StaticBook`、`BookBuilder` と旧 v2 変換 API を Rust / Python から削除。v2 単独から局面を復元できないため、元の棋譜や外部定跡から v4 を再生成する。外部 reader と所有 IR は維持する。
- Native 候補手の `count` を削除。外部入力の回数は 0 を含め欠落として報告し、既定の strict 変換は失敗する。明示的な lossy 変換だけが回数を捨てて続行する。
- ライブラリは minor 更新だが、定跡の API と保存形式に破壊的変更を含む。reader は wire version 4（magic `RSSHGBK4`）を受理し、旧 v2 と固定長 v3 を拒否する。固定長 v3 の互換 reader は設けず、入力から再生成する。利用側は `depth_kind == 1` の場合だけ深さを探索深さの条件に使う。

### Fixed

- `check_candidate_bb` の駒種と候補表の対応を修正した。index 6 の表は馬の候補であり、
  これまで `PieceType::ROOK` を渡したときに返されていた。`PieceType::HORSE` を渡すと
  馬の候補表を返すようになり、`PieceType::ROOK` / `PieceType::DRAGON` は
  「絞り込めない」ことを表す `Bitboard::ALL` を返す。従来の飛は馬の候補表、龍は
  `Bitboard::EMPTY` を返しており、単独で使うと王手を取りこぼす可能性があった。
  成駒（と金・成香・成桂・成銀）は金の候補表を返すようになった。
  関数が直接王手だけを覆い、開き王手を覆わないことを doc comment に明記した。
  **利用側の対応**: 馬の候補を得るために `PieceType::ROOK` を渡していた箇所は
  `PieceType::HORSE` に置き換える。置き換えない場合も `Bitboard::ALL` は上位集合なので
  結果の正しさは保たれるが、絞り込みが効かなくなる。

### Performance

- `generate_moves::<Checks>` などの `MoveGenType` 経由の王手生成が、非王手局面では
  王手候補表を使う専用生成器へ委譲するようになった。従来は全空きマス × 全駒種に対して
  `gives_check_move` を個別に呼ぶ総当たりだった。生成される指し手集合は変わらない
  （非王手の 79,233 局面で `Checks` / `ChecksAll` / `QuietChecks` / `QuietChecksAll` を
  1.4.0 および独立した参照経路と照合）。王手中の挙動は従来どおり王手回避手に絞る。
- `CapturePlusPro` / `CapturePlusProAll` の盤上手生成で、取る手にならない空きマスを
  `pop_lsb` で走査する前に Bitboard で落とすようにした。非歩駒は敵駒のある升だけ、
  歩は敵駒のある升と敵陣だけを走査する。生成される指し手集合は変わらない
  （82,257 局面で 1.4.0 および全候補からの抽出結果と一致）。
- 王手中の `solve_mate_in_one` が、作業局面の複製にキー・キャッシュの全再計算を
  含まなくなった。

### Development

- 王手生成の回帰テストは、王手候補表を使わず全候補から王手を選ぶ参照経路と照合する。
- 通常 CI に release ビルドのゼロアロケーション検査を追加し、debug ビルドで省略される一手詰めの確保検査を行う。
- feature 単独のビルドと、警告をエラーにする rustdoc 検査を追加する。
- 指し手の捕獲駒情報、特殊手定数、Python API 一覧と CI 構成の説明を実装に揃える。

## [1.4.0] - 2026-10-17

SAZ2 の保存形式と Rust の網羅的な `match` に互換性上の注意がある。

### Added

- `GameResult::DrawByAdjudication`（数値 `14`）を追加。規則上の引き分けが成立していない局面で、
  外部の裁定により引き分けとした結果を表す。`is_draw()` が `true` を返し、`sbinpack` の
  `game_result_from_u8`、HCPE の `Draw`、PACK の `Draw`、JKF の `HIKIWAKE`、
  Python の `GameResult.DRAW_BY_ADJUDICATION` に対応させた。
  `GameResult` は `#[non_exhaustive]` ではないため、利用側で全 variant を列挙した Rust の `match` は
  新しい variant への対応が必要になる。既存 variant の数値コードは変更しない。
  棋譜形式ごとの読み戻しと情報損失は [Python マニュアル](docs/book/src/python/record.md#裁定引き分けの保存) を参照。
- `SazTerminationReason::Resignation`（`8`）と `SazTerminationReason::Adjudication`（`9`）を追加。
  どちらも終局局面に合法手が残る終局で、`NoLegalMoves` や `RepetitionDraw` へ偽装せずに記録できる。
  `8` は敗者側が投了した終局であり、評価値の閾値で手番側の負けを確定させる自己対局の打ち切りを含む。
  `9` は投了を伴わない宣言で、引き分けを宣言した場合の `game_result` は `DrawByAdjudication` になる。

### Changed

- **Breaking:** 未使用の公開エラー variant `SazSelfplayError::PackedSfen`（`SazError::PackedSfen`）を削除。
  SAZ2 codec は開始局面のバイト列を保持し、局面としての妥当性は検証しない。
- **Breaking: SAZ2 自己対局形式を version 4 にする。** ワイヤーレイアウトと既存の終局理由 `0..=7` の意味は
  version 3 と同一だが、対局結果と終局理由に新しい値を追加したことをヘッダの版で区別する。
  reader は version 4 だけを受理し、version 3 のチャンクは `UnsupportedVersion(3)` で拒否する。
  version 3 で保存した既存データはそのままでは読めないため、変換または再生成が必要である。
  変換ツールは同梱しない。旧コードが裁定を別の終局理由で記録していた場合、その意味はバイト列だけでは復元できない。

### Fixed

- KIF / KI2 の裁定による引き分けを、独自拡張の終局表記 `裁定引分` として読み書きするように修正。
  終局行とサマリーの両方に対応し、`DrawByAdjudication` を書き出して読み戻しても中断や未決着にならず、裁定理由を保持する。
  標準の終局表記ではないため、他ソフトでの結果分類は保証しない。
  `SpecialMove::Draw` でも結果が `DrawByImpasse` の場合は `持将棋` として出力し、
  CSA `%HIKIWAKE` / JKF `HIKIWAKE` からの変換で裁定引分に変わらないようにした。
- Python マニュアルの `GameResult` 定数表が、実際の数値コードおよび定数名と一致していなかったのを修正。
  存在しない `BLACK_WIN_BY_PERPETUAL_CHECK` などを削除し、`from_str` の `"DRAW"` エイリアスと
  `is_draw()` の対象値の記述も実装に合わせた。
- SAZ2 の文書に残っていた終局理由の上限値と、局面や着手の合法性を検証するという記述を修正。
  codec が検証するのはバイナリ構造、列挙値、分布の合計であり、終局結果と終局理由の整合性は利用側が確認する。
- Windows の `make clean` が Python キャッシュを探索する際、reparse point、`.git`、`work_dir` を巡回しないようにした。

## [1.3.1] - 2026-10-17

SBK 定跡書庫のオープン性能を改善したパッチリリース。公開 API と保存形式に変更はない。

### Changed

- `SbkBook::open` / `open_with_control` がインデックス構築中にファイル全体をメモリへ読み込むように
  なった。ステートごとの `seek` と、グラフ走査中の `Position` クローンがなくなり、
  走査は `PackedSfen` をキーに単一の `Position` を再利用する。
  オープン時のピークメモリはファイルサイズに比例する（読み込んだバイト列は戻り値には残らない）。
  合成 107 MB SBK（840,000 state）での計測で、open は 34.5 s から 1.4 s、ピーク RSS は
  22.2 GB から 282 MB になった。

### Fixed

- `PackedSfen` で表現できない局面を含む SBK を開いたときに panic していたのを修正。
  `BookError::InvalidData` を返す。

## [1.3.0] - 2026-10-11

このリリースは minor 更新ですが、SAZ2 の公開 API と保存形式に破壊的変更を含みます。
SAZ2 を使う場合は、下記の変更点と互換性を確認してから更新してください。

### Added

- `Position::repetition_state_with_distance(depth)` を追加。`depth` 手以内で最も近い同一盤面の
  反復分類と、その局面までの手数を返す。探索木の根からの深さを渡すと、根以降に閉じた反復だけを
  周期とともに得られる。4 回目の同一局面でも `depth` の外側の一致は返さない。
  `repetition_state_with_ply` は同じ走査を共有し、結果は変わらない。

### Changed

- **Breaking: SAZ2 自己対局形式を version 3 にする。** policy entry は探索終了時に root が
  その手へ保持していた累積訪問数 `visits` を 1 つ持つ。`visits_before` / `visits_after` と、
  その単調性を検査する `SazSelfplayError::DecreasingVisits` を削除した。木の再利用で引き継いだ
  訪問を探索中に取り消す探索では、2 つの訪問数の差が今回の探索の寄与を表さないためである。
- `SazSelfplayPolicyEntry` の field が変わったため、struct literal は修正が必要になる。
  Python の `SazPolicyEntry(...)` は `visits` を受け取り、`visits_before` / `visits_after`
  property を持たない。

### Compatibility

- version 2 の chunk は `UnsupportedVersion` で拒否し、変換経路はない。version 2 のデータを
  読むには 1.2.x を使う。version 3 の `visits` は version 2 の `visits_after` と同じ量であり、
  累積訪問数から policy 教師を作っていた場合は教師の値が変わらない。

## [1.2.5] - 2026-09-25

### Fixed

- KI2 の成香・成桂・成銀の指し手表記を二文字に戻す。従来の杏・圭・全も読み込める。

## [1.2.4] - 2026-09-10

### Added

- SVG 描画に `SvgOptions` と `Position::to_svg_with_options()` を追加。
  Python の `Board.to_svg()` でも、升の着色、移動先の駒の太字、盤内の背景色を独立して指定できる。
  既存の呼び出しと既定表示は維持する。

### Fixed

- 不正な指し手、駒情報の不整合、カウンターの上限超過を局面更新前に検出し、
  エラー時に局面と履歴を保持する。`MoveError` に `InvalidMove` と `CounterOverflow` を追加。
- SFEN の解析失敗時に局面を保持し、SBK と YBB の不正入力をエラーとして返す。
  SBK では重複した局面 ID と不正な指し手も検出する。
- Windows の Rust テスト実行で、子プロセスの失敗を終了コードに反映する。
- Rust と Python の使用例、feature の説明、棋譜と学習データ形式の文書を現行 API に合わせて修正。

## [1.2.3] - 2026-08-18

### Added

- Added `Position::try_to_huffman_coded_pos` for fallible HCP encoding of externally supplied or
  otherwise unvalidated positions.
- Validation reports now identify inventories that exceed the standard total for each piece type.

### Changed

- Python record parsers release the GIL while parsing text, and PSV export traverses deep record
  trees iteratively.
- Repetition adjudication examines the retained game history without applying the search clone's
  16-ply history limit. `Position::clone_for_search` also retains the full post-null history by
  default, preserving long-period repetition adjudication after cloning.

### Fixed

- Move legality rejects malformed raw moves, dead-end drops, and unpromoted knight moves to ranks
  where the knight would have no legal destination. Applying a piece-less `Move32` now restores
  its board metadata before updating check state.
- SFEN, static-book, SAZPACK, SBINPACK, KIF, KI2, CSA, JKF, YaneuraOu book, HCP, and PACK paths now
  reject malformed counts, lengths, offsets, overflows, or inconsistent results instead of
  panicking, allocating from untrusted headers, truncating, or silently corrupting data.
- HCP, PackedSfen, and PackedSfenValue encoding now reject one-king and excess-inventory positions
  through fallible Rust and Python APIs before writing past the 32-byte bitstream.
- KI2 emits combined direction modifiers such as `左上` when required for an unambiguous
  round-trip. KIF and KI2 also preserve exported time controls, comments, branches, and handicap
  side names more accurately.
- Record merges are atomic on validation failure, deep equality and subtree operations are
  iterative, and safe undo rejects a move that does not match the current state.
- Default-feature Rust builds are checked in CI in addition to the all-features configuration.

## [1.2.2] - 2026-08-18

### Added

- Added `Ki2Notation`, a compact, allocation-free value for one canonical KI2 move notation. It
  supports parsing, formatting, hashing, and resolution to a legal `Move32` in a position.
- Added `Move::to_ki2_notation` and `Move32::to_ki2_notation` for callers that need to compare or
  aggregate KI2 notation without allocating a `String`.

### Changed

- `Move::to_ki2` and `Move32::to_ki2` now format from `Ki2Notation`, keeping string output and the
  structured representation on one implementation path while allocating only the returned string.
- KI2 formatting keeps board-origin candidates as a `Bitboard` instead of allocating a temporary
  `Vec<Move>` for every formatted move.

### Fixed

- KI2 formatting now continues ambiguity resolution when multiple same-type pieces can move
  sideways (`寄`) to the same destination. It adds `右` or `左` when needed instead of emitting
  identical notation for distinct legal moves.
- KI2 parsing now recognizes BOD headers before move lines, allowing records with custom initial
  positions to round-trip through `export_ki2` and `parse_ki2_str`.

## [1.2.1] - 2026-08-18

### Added

- `mate::solve_mate_in_one_in_place` takes `&mut Position` and searches without duplicating the
  position. It applies each candidate to the given position and undoes it, so the position is
  restored when the function returns normally. The caller must have initialized the state stack.
  As with the other mutating APIs, the state of the position after a panic is unspecified.
- `MoveSink::stop` and `Move32Sink::stop` let a sink request early termination. The default
  implementation returns `false`, so existing sinks continue to collect every generated move.
  Termination is cooperative: generators poll at implementation-defined checkpoints rather than
  after every emitted move. The four legal-evasion `*_into` APIs stream checked positions through
  `Legal` or `LegalAll`, and leave their sinks unchanged on non-check positions.

### Changed

- Mate-in-one detection discards candidates without applying them when a legal reply to the check
  can be constructed statically: a king escape, a capture of the checking piece, or an
  interposition. A candidate is discarded only when such a reply is found, so a mate is never
  missed; mate itself is still confirmed by generating the defender's legal moves. Pawn drops are
  skipped outright because a mate by dropping a pawn is illegal.
- On positions where the side to move is not in check, `mate::solve_mate_in_one` streams checking
  drops before checking board moves and stops after proving the first mate. This may select a
  different move when more than one legal mate exists; the returned move is still guaranteed to
  be legal, give check, and leave the defender without a legal reply.
- `Position::is_mated` stops generating as soon as it finds one legal move instead of collecting
  the whole legal set.
- In the final native-build qualification over 1.99 million positions, the immutable mate-in-one
  API was `45.4%` faster on the mate1 corpus and `5.6%` faster on the mate3 corpus than the
  list-based pre-streaming implementation from the same release candidate.

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

[Unreleased]: https://github.com/nyoki-mtl/rsshogi/compare/v1.6.0...HEAD
[1.6.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.5.1...v1.6.0
[1.5.1]: https://github.com/nyoki-mtl/rsshogi/compare/v1.5.0...v1.5.1
[1.5.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.4.0...v1.5.0
[1.4.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.3.1...v1.4.0
[1.3.1]: https://github.com/nyoki-mtl/rsshogi/compare/v1.3.0...v1.3.1
[1.3.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.5...v1.3.0
[1.2.5]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.4...v1.2.5
[1.2.4]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.3...v1.2.4
[1.2.3]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.2...v1.2.3
[1.2.2]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.1...v1.2.2
[1.2.1]: https://github.com/nyoki-mtl/rsshogi/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/nyoki-mtl/rsshogi/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/nyoki-mtl/rsshogi/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.1.0
[1.0.2]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.2
[1.0.1]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.1
[1.0.0]: https://github.com/nyoki-mtl/rsshogi/releases/tag/v1.0.0
