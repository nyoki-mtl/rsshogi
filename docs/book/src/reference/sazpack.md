# sazpack フォーマット仕様（SAZ2 v3）

`sazpack`（マジック `SAZ2`、現行バージョン `3`）は、AlphaZero 系のモンテカルロ木探索（MCTS）自己対局から得られる教師データを対局単位のチャンクとして保存するバイナリフォーマットである。
各対局の開始局面と終局情報に加え、各手番における探索後の root WDL、ネットワークの raw 出力（raw WDL、raw prior、raw mate、raw moves-left）、証明詰み手数、および合法手ごとの訪問回数と勝敗上下限（疎な policy 分布）を格納する。

## 1. ワイヤーレイアウト仕様

`sazpack` チャンクは、10 バイトの固定長チャンクヘッダと、連続配置された対局レコード列（`SazSelfplayGame*`）で構成される。
固定幅のマルチバイト整数はすべてリトルエンディアン（LE）であり、要素数と訪問回数には可変長整数 ULEB128（`u32` 上限）を使用する。

```text
Chunk = ChunkHeader (10 bytes) + SazSelfplayGame*
```

### チャンクヘッダ（10 バイト）

| オフセット | サイズ | エンコーディング | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 4B | `[u8; 4]` | `magic` | `b"SAZ2"` 固定。不一致時は `SazSelfplayError::InvalidMagic`。 |
| `0x04` | 1B | `u8` | `version` | `3` 固定。`3` 以外の値は `SazSelfplayError::UnsupportedVersion(version)`。 |
| `0x05` | 1B | `u8` | `flags` | `0` 固定。`0` 以外の値は `SazSelfplayError::UnsupportedFlags(flags)`。 |
| `0x06` | 4B | `u32` LE | `payload_len` | ヘッダ直後に続くペイロード全体のバイト長。ファイル長が `10 + payload_len` より短い場合は `Truncated`、長い場合は `TrailingBytes`。 |

### 対局レコードのワイヤーレイアウト（`SazSelfplayGame` / `SazGame`）

チャンクペイロード内では、以下の順序で 1 局分のデータが直列化される。

| 順序 | サイズ | エンコーディング | フィールド名 | 内容と有効値 |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 32B | `[u8; 32]` | `stem_packed_sfen` | 対局開始局面の 32 バイト `PackedSfen`。デコード時に `Position::set_packed_sfen` で合法性が検証される。 |
| 2 | 1B | `u8` | `game_result` | 対局結果 `GameResult` の数値表現（未定義値は `InvalidGameResult`）。 |
| 3 | 1B | `u8` | `termination_reason` | 終局理由 `SazTerminationReason`（`0`：合法手なし、`1`：千日手引分、`2`：連続王手の千日手勝ち、`3`：連続王手の千日手負け、`4`：優等局面、`5`：劣等局面、`6`：入玉宣言、`7`：最大手数到達）。`8` 以上は `InvalidTerminationReason`。 |
| 4 | 1B | `u8` | `entering_king_rule` | 入玉宣言ルール `EnteringKingRule`（`0`：`None`、`1`：`Point24`、`2`：`Point24Handicap`、`3`：`Point27`、`4`：`Point27Handicap`、`5`：`TryRule`、`6`：`Unset`）。`7` 以上は `InvalidEnteringKingRule`。 |
| 5 | `1..=5` B | ULEB128 (`u32`) | `position_count` | この対局に含まれる局面レコードの個数 $P$。 |
| 6 | 可変長 | `SazSelfplayPosition * P` | `positions` | $P$ 個の局面レコードを順に連続格納する。 |

### 局面レコードのワイヤーレイアウト（`SazSelfplayPosition` / `SazPosition`）

各局面レコードは、34 バイト（`mate` が `Some` の場合は 36 バイト）の固定順序ヘッダに続けて、ULEB128 の `policy_count` と可変長の `policy` エントリ列を配置する。

| 順序 | サイズ | エンコーディング | フィールド名 | 内容と量子化スケール |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 2B | `u16` LE | `played` | この局面で実際に選択された 16-bit 指し手（`Move::raw()`）。 |
| 2 | 6B | `u16` LE $\times 3$ | `root_wdl` | MCTS 探索後の root 集約 WDL（`win`, `draw`, `loss` の順）。3 要素の合計は厳密に `65535`。 |
| 3 | 6B | `u16` LE $\times 3$ | `outcome_wdl` | 終局結果から確定した教師 WDL（`win`, `draw`, `loss` の順、手番視点）。合計は厳密に `65535`。 |
| 4 | 6B | `u16` LE $\times 3$ | `raw_wdl` | 探索を通す前にネットワークが出力した WDL（`win`, `draw`, `loss` の順）。合計は厳密に `65535`。 |
| 5 | 2B | `u16` LE | `raw_mate` | ネットワークの mate head が出力した詰み確率（`u16 / 65535` 量子化）。 |
| 6 | 2B | `u16` LE | `raw_moves_left` | ネットワークの moves-left head の予測残り手数（`手数 * 32` の固定小数点数、分解能 $1/32$ 手）。 |
| 7 | 2B | `u16` LE | `plies_left` | 実際の対局終了までの残り手数。 |
| 8 | 4B | `u32` LE | `requested_visits` | この局面で要求された探索シミュレーション回数。 |
| 9 | 2B | `u16` LE | `target_weight_milli` | 学習時のサンプル重み（千分率、`1000` = 重み $1.0$）。 |
| 10 | 1B | `u8` | `exploration_flags` | 探索設定やノイズ有無を表すビットフラグ。 |
| 11 | 1B | `u8` | `mate_tag` | 証明詰み手数 `mate` の有無タグ（`0`：`None`、`1`：`Some`）。`0` と `1` 以外は `InvalidMateTag`。 |
| 12 | 0B または 2B | `i16` LE（条件付き） | `mate_value` | `mate_tag == 1` のときのみ存在する 2 バイト符号付き詰み手数。`mate_tag == 0` のときは 0 バイト。 |
| 13 | `1..=5` B | ULEB128 (`u32`) | `policy_count` | 後続する `policy` エントリの個数 $M$。 |
| 14 | 可変長 | `SazSelfplayPolicyEntry * M` | `policy` | $M$ 個の候補手 policy エントリを連続格納する。 |

### Policy エントリのワイヤーレイアウト（`SazSelfplayPolicyEntry` / `SazPolicyEntry`）

各候補手（最小 9 バイト）は以下の順序で直列化される。

| 順序 | サイズ | エンコーディング | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 2B | `u16` LE | `mv` | 16-bit 指し手（`Move::raw()`）。学習時には手番と合わせて `CompactMoveLabel`（`0..1496`）または `MoveLabel`（`0..2187`）へ変換して利用できる。 |
| 2 | 2B | `u16` LE | `prior` | 探索が実際に使用した事前確率（Dirichlet ノイズ等適用後、量子化値 `0..=65535`）。 |
| 3 | 2B | `u16` LE | `raw_prior` | ネットワークが出力したそのままの事前確率（量子化値 `0..=65535`）。 |
| 4 | `1..=5` B | ULEB128 (`u32`) | `visits` | 探索終了時に root がこの候補手へ保持していた累積訪問回数（ツリー再利用による引き継ぎ分を含む）。 |
| 5 | 1B | `u8` | `lower` | 探索で証明された勝敗下限 `SazOutcomeBound`（`0`：`Loss`、`1`：`Draw`、`2`：`Win`）。`3` 以上は `InvalidOutcomeBound`。 |
| 6 | 1B | `u8` | `upper` | 探索で証明された勝敗上限 `SazOutcomeBound`（`0`：`Loss`、`1`：`Draw`、`2`：`Win`）。`3` 以上は `InvalidOutcomeBound`。 |

> [!NOTE]
> メモリ上や下流の学習テンソル構築において、候補手のラベルと勝敗上下限を 16-bit 整数 `packed_label_bounds: u16` にパックして扱う場合は、下位 11 ビット（bits `0..=10`）に `CompactMoveLabel`（`0..1496`）、bits `11..=12` に `lower`（`SazOutcomeBound`）、bits `13..=14` に `upper`（`SazOutcomeBound`）、bit `15` に `0` を割り当てる設計と対応付けられる。`SAZ2` v3 チャンクのワイヤー上では、局面再生なしで直接読み書きできるよう `mv: u16 LE`（2B）と `lower: u8`（1B）、`upper: u8`（1B）として記録される。

## 2. 検証不変条件（Validation Invariants）

`serialize_chunk`（書き出し）と `deserialize_chunk`（読み込み）の双方で、以下の不変条件が厳密に検証される。

1. **WDL 分布の総和**：各局面の `root_wdl`、`outcome_wdl`、`raw_wdl` は、いずれも `u32::from(win) + u32::from(draw) + u32::from(loss) == 65535`（`SazWdl::TOTAL`）を満たさなければならない。不一致の場合は `SazSelfplayError::InvalidDistributionSum` を返す。
2. **Prior 分布の総和**：`policy` が空でない各局面について、全エントリの `prior` の合計および `raw_prior` の合計がそれぞれ厳密に `65535` に一致しなければならない（桁溢れを防ぐため残余予算 `65535` からの減算 `checked_sub` で検証される）。なお、`policy` が 0 件の終端局面等では合計が `65535` に達しないため、`policy` は少なくとも 1 手以上のエントリを持ち合計 `65535` を満たす必要がある。
3. **列挙値とタグの境界**：未知の `GameResult`、`SazTerminationReason`（`> 7`）、`EnteringKingRule`（`> 6`）、`SazOutcomeBound`（`> 2`）、および `mate_tag`（`0` または `1` 以外）はすべてデコード時に拒否される。

## 3. コード例

### Python

Python では `rsshogi.sazpack` サブモジュール（およびトップレベル `rsshogi`）で `SazWdl`、`SazPolicyEntry`、`SazPosition`、`SazGame` と読み書き関数を公開している。

```python
from rsshogi import Board, GameResult
from rsshogi.sazpack import (
    SazGame,
    SazPolicyEntry,
    SazPosition,
    SazWdl,
    decode_sazpack,
    decode_sazpack_file,
    write_sazpack,
    write_sazpack_file,
)

board = Board()
wdl = SazWdl(win=30000, draw=5535, loss=30000)  # 合計 65535
outcome = SazWdl(win=65535, draw=0, loss=0)

pos = SazPosition(
    played="7g7f",
    root_wdl=wdl,
    outcome_wdl=outcome,
    raw_wdl=wdl,
    raw_mate=0,
    raw_moves_left=3200,  # 100.0 手 (100 * 32)
    plies_left=95,
    requested_visits=800,
    target_weight_milli=1000,
    exploration_flags=1,
    policy=[
        SazPolicyEntry(mv="7g7f", prior=40000, raw_prior=40000, visits=500, lower=0, upper=2),
        SazPolicyEntry(mv="2g2f", prior=25535, raw_prior=25535, visits=300, lower=0, upper=2),
    ],
    mate=None,
)

game = SazGame(
    stem=board.to_sfen(),
    game_result=GameResult.BLACK_WIN,
    termination_reason=0,  # NoLegalMoves
    entering_king_rule=3,  # Point27
    positions=[pos],
)

# バイト列およびファイルへの書き出しとデコード
chunk_bytes = write_sazpack([game])
decoded_games = decode_sazpack(chunk_bytes)

write_sazpack_file("selfplay.sazpack", [game])
loaded_games = decode_sazpack_file("selfplay.sazpack")
```

### Rust

Rust では `serialize_chunk` / `deserialize_chunk` に加え、チャンク内における各対局のバイト範囲（`Range<usize>`）を同時に取得できる `deserialize_chunk_indexed` を提供している。

```rust
use rsshogi::records::formats::sazpack::{
    IndexedSazSelfplayGame, SazGame, SazOutcomeBound, SazPolicyEntry, SazPosition,
    SazTerminationReason, SazWdl, deserialize_chunk, deserialize_chunk_indexed, serialize_chunk,
};

let bytes = serialize_chunk(&games)?;
let decoded: Vec<SazGame> = deserialize_chunk(&bytes)?;

// チャンク内の各対局が占める payload バイト範囲を取得する
let indexed: Vec<IndexedSazSelfplayGame> = deserialize_chunk_indexed(&bytes)?;
for item in &indexed {
    let slice = &bytes[item.payload_range.clone()];
    println!("game positions={}, byte_len={}", item.game.positions.len(), slice.len());
}
```

## 関連項目

- [学習用バイナリ仕様（HCP / HCPE / PackedSfen / PSV / PACK）](formats/training-binaries.md)
- [sbinpack フォーマット仕様](sbinpack.md)
- [Policy ラベルの設計と変換](../internals/types/policy-labels.md)
