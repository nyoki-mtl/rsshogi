# sbinpack フォーマット仕様（SBN2）

`sbinpack`（Shogi binpack、マジック `SBN2`、現行バージョン `2`）は、Stockfish の NNUE 訓練データ形式である binpack を将棋向けに適応した高圧縮バイナリフォーマットである。
開始局面（`SbinpackStem`）と、それに続く連続した指し手および評価値列（チェーン）を、合法手オーダリングインデックスとパリティ正規化差分で可変長符号化して保存する。

## 設計の特徴と Stockfish binpack との比較

| 項目 | Stockfish binpack（チェス） | `sbinpack` v2（将棋） |
| :--- | :--- | :--- |
| チャンクマジック | `BINP` | `SBN2` |
| 先頭局面表現 | 24 バイト | 32 バイト `PackedSfen` |
| Stem サイズ | 32 バイト | 38 バイト（`SbinpackStem`） |
| エンディアン | ビッグエンディアン | リトルエンディアン（LE） |
| 可変長整数符号化 | 4-bit + 拡張ビット | ULEB128（符号付き差分は ZigZag + ULEB128） |
| チェーン単位メタデータ | なし | `MetadataLen` (`0..=127`) + opaque バイト列 |

## 1. ワイヤーレイアウト仕様

`sbinpack` ファイルは 1 個以上のチャンク（`Chunk`）を連結したストリームであり、各チャンクは 8 バイトのチャンクヘッダと 0 個以上のチェーン（`Chain`）で構成される。

```text
File  = Chunk*
Chunk = ChunkHeader (8 bytes) + Chain*
Chain = SbinpackStem (38 bytes)
      + MetadataLen (1 byte, 0..=127)
      + Metadata (MetadataLen bytes)
      + Count (u16 LE, 2 bytes)
      + (EncodedMove (ULEB128) + EncodedScore (ZigZag + ULEB128)) * Count
```

### チャンクヘッダ（8 バイト）

| オフセット | サイズ | 型 | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 4B | `[u8; 4]` | `magic` | `b"SBN2"` 固定。不一致時は `SbinpackError::InvalidMagic`。 |
| `0x04` | 4B | `u32` LE | `chunk_size` | このチャンクヘッダ直後に続くペイロード（`Chain*`）の合計バイト数。 |

（デコード後の `SbinpackFile` 構造体は `version = 2` および `flags = 0` を保持する。）

### チェーン構造（`SbinpackChain`）

1 つのチェーンは開始局面から連続する手順ライン（本手順または 1 つの変化手順）を表す。

| 順序 | サイズ | 型 | フィールド名 | 内容と検証規則 |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 38B | `SbinpackStem` | `stem` | チェーン先頭局面の 38 バイト固定長ヘッダ（下表参照）。 |
| 2 | 1B | `u8` | `metadata_len` | `metadata` のバイト長（`0..=127`）。`128` 以上の値は `SbinpackError::MetadataTooLarge`。 |
| 3 | `0..=127` B | `[u8]` | `metadata` | 利用側が任意に定義できるチェーン単位の opaque バイト列（`metadata_len == 0` のときは 0 バイト）。 |
| 4 | 2B | `u16` LE | `count` | 後続する `MoveText` に含まれる指し手の総数 $K$。 |
| 5 | 可変長 | `(ULEB128, ULEB128) * K` | `moves` | 1 手ごとに `EncodedMove`（合法手インデックスの ULEB128）と `EncodedScore`（評価値差分の ZigZag + ULEB128）を交互に $K$ 回繰り返す。 |

### 先頭局面ヘッダ（`SbinpackStem`、38 バイト）

`SbinpackStem` は `sbinpack` 固有の 38 バイト構造体であり、40 バイトの `PackedSfenValue`（PSV）とはレイアウトが異なる。

| オフセット | サイズ | 型 | フィールド名 | 内容 |
| :--- | :--- | :--- | :--- | :--- |
| `0x00` | 32B | `[u8; 32]` | `packed_sfen` | チェーン先頭局面の 32 バイト `PackedSfen`。 |
| `0x20` | 2B | `i16` LE | `score` | 先頭局面の評価値（手番視点、単位：cp）。 |
| `0x22` | 2B | `u16` LE | `best_move_u16` | 先頭局面の 16-bit 指し手（`rsshogi::Move::raw()`）。 |
| `0x24` | 2B | `u16` LE | `ply_result` | 下位 10 ビット（bits `0..=9`）が先頭局面の手数 `ply`（`0..=1023`、超過時は `1023` に飽和）、上位 6 ビット（bits `10..=15`）が `GameResult` の数値表現（`0..=63`）。 |

### `Record` との相互変換における `SbinpackStem` の扱い

- **`Record` から `sbinpack` への変換時**：チェーンの指し手列 `moves` が 1 手以上ある場合は先頭手 `moves[0].mv` が `stem.best_move_u16` に格納され、0 手（空手順）の場合は `Move::MOVE_NONE`（`0`）が格納される。`stem.score` には呼び出し側が指定した `stem_score`（既定値 `0`）が格納される。
- **`sbinpack` から `Record` への変換時**：`stem.packed_sfen` と `ply_result` から初期局面と対局結果が復元されるが、`stem.score` および `stem.best_move_u16` は `Record` 側にはコピーされない（各手の評価値は `MoveText` 内の `EncodedScore` から復元される）。

## 2. 指し手の符号化（`EncodedMove`）と決定的オーダリング

各局面における指し手は、`MoveListGen::<LegalAll>` で生成されたすべての通常合法手を以下の **v1.0.0 決定的オーダリング規則** でソートした際の 0 始まりインデックス（`u16`）を ULEB128 で可変長符号化したものである。

### 合法手ソートキーの優先順位（昇順）

1. **駒打ち判定**：盤上移動（`0`） $\to$ 駒打ち（`1`）。
2. **捕獲判定**：非捕獲の静かな手（`0`） $\to$ 駒を取る手（`1`）。
3. **成り判定**：不成（`0`） $\to$ 成り（`1`）。
4. **移動前（または打つ）駒種の優先順位**：
   `歩 (0)` $\to$ `香 (1)` $\to$ `桂 (2)` $\to$ `銀 (3)` $\to$ `金 (4)` $\to$ `角 (5)` $\to$ `飛 (6)` $\to$ `玉 (7)` $\to$ `と (8)` $\to$ `成香 (9)` $\to$ `成桂 (10)` $\to$ `成銀 (11)` $\to$ `馬 (12)` $\to$ `龍 (13)`。
5. **移動先（`to`）の段**：手番視点で正規化（後手番は盤面を 180 度回転）した後の段で、敵陣に近い前方段を優先（1 段目 $\to$ 9 段目）。
6. **移動先（`to`）の筋**：手番視点で正規化した後の筋で、中央筋を優先（`5, 4, 6, 3, 7, 2, 8, 1, 9` 筋の順）。
7. **移動元（`from`）の段**：盤上移動について、手番視点で正規化した後の前方段を優先（1 段目 $\to$ 9 段目）。
8. **移動元（`from`）の筋**：盤上移動について、手番視点で正規化した後の中央筋を優先（`5, 4, 6, 3, 7, 2, 8, 1, 9` 筋の順）。
9. **タイブレーク**：`mv.to_move().raw()` の昇順。

出現頻度の高い手（歩や銀の前進、中央付近の着手など）に小さいインデックスが割り当てられるため、ほとんどの合法手インデックスが 1 バイト（`0..=127`）の ULEB128 に収まる。

## 3. 評価値差分の符号化（`EncodedScore`）

各指し手 `moves[i]` の評価値 `move_eval_i`（`i32`、その手を指す直前の手番視点）は、手番の交代による符号反転を打ち消すパリティ正規化を行ったうえで、直前の値との差分を ZigZag + ULEB128 で符号化する。

$$\text{norm}_i = \begin{cases} \text{move\_eval}_i & (i \text{ が偶数}) \\ -\text{move\_eval}_i & (i \text{ が奇数}) \end{cases}$$

$$\Delta_0 = \text{norm}_0 - \text{stem.score}, \qquad \Delta_i = \text{norm}_i - \text{norm}_{i-1} \quad (i \ge 1)$$

$$\text{EncodedScore}_i = \text{ULEB128}(\text{ZigZag}(\Delta_i))$$

ここで 32-bit 符号付き整数 $x$ に対する ZigZag 変換と逆変換は以下の通りである。

$$\text{zigzag}(x) = ((x \ll 1) \oplus (x \gg 31)) \text{ as } \text{u32}, \qquad \text{unzigzag}(u) = ((u \gg 1) \text{ as } \text{i32}) \oplus -((u \mathbin{\&} 1) \text{ as } \text{i32})$$

形勢が急変しない通常の対局では、パリティ正規化後の隣接評価値差分 $\Delta_i$ が $0$ 付近の小さな値に集中するため、高い圧縮率が得られる。

## 4. コード例

### Python

Python では `Record` のメソッドに加えて、`rsshogi.record` モジュール（およびトップレベル `rsshogi`）で複数局やファイル単位の読み書き関数を提供している。

```python
from rsshogi import EngineInfo, GameResult, MoveEntry, Record, SpecialMoveEntry
from rsshogi.record import (
    decode_sbinpack,
    decode_sbinpack_file,
    write_sbinpack,
    write_sbinpack_file,
)

# 評価値付きの Record を構築する（sbinpack 変換には全手の評価値が必須）
record = Record.from_main_line(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    [
        MoveEntry("7g7f", engine_info=EngineInfo(eval=110)),
        MoveEntry("3c3d", engine_info=EngineInfo(eval=-115)),
    ],
    SpecialMoveEntry("RESIGN", GameResult.WHITE_WIN),
)

# 1. 単一 Record と sbinpack バイト列の相互変換
data = record.to_sbinpack(
    stem_score=100,
    include_main=True,
    include_variations=False,
    metadata=b"game-001",
)
restored = Record.from_sbinpack(data)
restored_rec, meta = Record.from_sbinpack_with_metadata(data)

# 2. 複数 Record の一括シリアライズとファイル入出力
chunk_bytes = write_sbinpack([record], stem_score=100, metadata=b"batch-1")
pairs = decode_sbinpack(chunk_bytes)  # list[tuple[Record, bytes]]

write_sbinpack_file("train.sbinpack", [record], stem_score=100)
file_pairs = decode_sbinpack_file("train.sbinpack")
```

### Rust（ゼロコピー逐次デコーダ `SbinpackDecoder`）

Rust の `SbinpackDecoder` は、ファイル全体を `Vec<SbinpackChain>` に展開することなく、チャンクとチェーンを先頭から走査して各指し手適用前の `&Position` をゼロコピーでコールバックへ貸し出す。
また、`decode_next_controlled_with` で `ChainStart` イベント時に `SbinpackDecodeControl::SkipChain` を返すと、`PackedSfen` の復元や合法手生成を行わずにそのチェーンの可変長ペイロードを高速にスキップできる。

```rust
use rsshogi::records::formats::sbinpack::{
    SbinpackDecodeControl, SbinpackDecodeEvent, SbinpackDecoder, SbinpackError,
};
use rsshogi::types::GameResult;

fn scan_sbinpack(bytes: &[u8]) -> Result<usize, SbinpackError> {
    let mut decoder = SbinpackDecoder::new(bytes);
    let mut visited_positions = 0usize;

    while let Some(step) = decoder.decode_next_controlled_with(|event| match event {
        SbinpackDecodeEvent::ChainStart { result, metadata, .. } => {
            // 例: 未確定局や特定のメタデータを持つチェーンをリプレイせずにスキップする
            if result == Some(GameResult::Invalid) || metadata.is_empty() {
                SbinpackDecodeControl::SkipChain
            } else {
                SbinpackDecodeControl::Continue
            }
        }
        SbinpackDecodeEvent::PositionBeforeMove { position, mv, eval, .. } => {
            let _ = (position.turn(), mv, eval);
            visited_positions += 1;
            SbinpackDecodeControl::Continue
        }
        _ => SbinpackDecodeControl::Continue,
    }) {
        step?;
    }
    Ok(visited_positions)
}
```

## 関連項目

- [学習用バイナリ仕様（HCP / HCPE / PackedSfen / PSV / PACK）](formats/training-binaries.md)
- [sazpack フォーマット仕様](sazpack.md)
- [Record と棋譜データモデル](records.md)
