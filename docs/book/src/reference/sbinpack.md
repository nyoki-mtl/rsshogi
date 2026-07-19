# sbinpack v2

## 概要

**sbinpack**（**S**hogi **binpack**）は、チェスの訓練データ形式である
[binpack](https://github.com/official-stockfish/Stockfish/blob/tools/docs/binpack.md)
を将棋向けに適応したバイナリフォーマットです。

オリジナルの binpack は Stockfish の NNUE 訓練データ用に設計され、実戦譜から生成された
連続局面を効率的に圧縮できます。sbinpack は同じ設計思想を将棋に適用し、
局面単位で扱える将棋版 binpack 形式として設計しました。

v2 の仕様として、`EncodedMove` のオーダリング、評価値差分、per-chain メタデータ拡張枠を定義します。

## 方針

- `sbinpack` をバイナリ棋譜の標準形式とします。

## 参考資料

- **オリジナル binpack 仕様**: [Stockfish binpack.md](https://github.com/official-stockfish/Stockfish/blob/tools/docs/binpack.md)
- **binpack 圧縮実装**: [nnue_data_compress](https://github.com/Sopel97/nnue_data_compress)

## 設計目標

- **仕様の安定性**: バージョンで順序が揺れないルールベースのオーダリングを採用する。
- **圧縮効率**: 頻出する手が小さいインデックスになりやすい順序を目指す。
- **説明可能性**: 将棋の自然な分類（打ち/非打ち、成り/非成り、駒種、座標）で決まる。
- **binpack 互換の構造**: Chunk / Chain / Stem / MoveText の階層構造を踏襲する。

## Python bindings (rsshogi)

```python
record.to_sbinpack(
    stem_score: int = 0,
    include_main: bool = True,
    include_variations: bool = False,
    metadata: bytes | None = None,
) -> bytes
Record.from_sbinpack(data: bytes) -> Record
Record.from_sbinpack_with_metadata(data: bytes) -> tuple[Record, bytes]
rsshogi.record.decode_sbinpack(data: bytes) -> list[tuple[Record, bytes]]
rsshogi.record.decode_sbinpack_file(path) -> list[tuple[Record, bytes]]
```

- `include_variations=True` の場合、metadata は出力される各 variation chain にコピーされるため、同じ bytes が 1 ファイル内に複数回現れ得ます。

- 本手順/分岐のすべての指し手に評価値が必要です（欠損時は `ValueError`）。
- `include_variations=True` の場合、分岐ごとに独立した chain を追加します（学習用）。
- `metadata` は 0..=127 bytes の per-chain opaque bytes です。未指定の場合は `MetadataLen=0` の 1 byte だけを出力します。
- `from_sbinpack` は `Record` だけを返します。opaque metadata も必要な場合は `from_sbinpack_with_metadata` を使います。
- `from_sbinpack` は **単一 chain** のみ対応します（複数 chain は `ValueError`）。

## Rust streaming decode

`SbinpackDecoder`は、file全体を`SbinpackFile`へmaterializeせず、chunk、chain、指し手適用前局面を1件ずつ通知します。入力にはborrowed `&[u8]`またはowned `Vec<u8>`を渡せます。

```rust
use rsshogi::records::formats::sbinpack::{
    SbinpackDecodeEvent, SbinpackDecoder, SbinpackError,
};

fn visit(bytes: Vec<u8>) -> Result<(), SbinpackError> {
    let mut decoder = SbinpackDecoder::new(bytes);
    while let Some(result) = decoder.decode_next_with(|event| {
        if let SbinpackDecodeEvent::PositionBeforeMove { position, eval, .. } = event {
            // positionは指し手適用前の局面。必要なowned値をここで取り出す。
            let _ = (position.turn(), eval);
        }
    }) {
        result?;
    }
    Ok(())
}
```

`decode_next_controlled_with`で`ChainStart`から`SkipChain`を返すと、`PackedSfen`とmove indexの合法性を検証せず、そのchainの可変長payloadを読み飛ばせます。結果種別やopaque metadataだけでchainを除外できるconsumer向けの高速経路です。通常の`decode_next_with`は全chainをreplayして検証します。

## EncodedMove の前提

- `LegalAll` で合法手を列挙し、その並びの **インデックス**を `EncodedMove` とする。
- インデックスは 0 起点（合法手最大 593 手）。
- インデックスの並びは **固定ルール**で決める（統計依存の再学習はしない）。

## ファイルフォーマット（v2）

オリジナルの binpack 形式（Chunk / Chain / Stem / MoveText の構造）を踏襲しつつ、
将棋固有の要素（PackedSfen、mv、合法手オーダリング）を組み込んでいます。

### binpack との主な違い

| 項目 | binpack (Chess) | sbinpack (Shogi) |
|------|-----------------|------------------|
| マジック | `BINP` | `SBN2` |
| 局面表現 | 24 bytes | PackedSfen (32 bytes) |
| 指し手 | 2 bytes | mv |
| Stem サイズ | 32 bytes | 38 bytes |
| エンディアン | Big-endian | Little-endian |
| 可変長整数 | 4bit+拡張ビット | ULEB128 |

### バイトオーダー

- すべて **リトルエンディアン**。
- 可変長整数は **ULEB128**。
- v2 は `SBN2` マジックで識別する。v1 (`SBIN`) との後方互換読み込みは提供しない。

### ルート構造

File = Chunk*

```text
Chunk = ChunkHeader + Chain*
ChunkHeader = "SBN2" + chunk_size(u32 LE)
```

### Chain（局面単位エントリ）

```text
Chain = Stem + MetadataLen + Metadata + Count + MoveText
Stem  = PackedSfenValue(38 bytes, LE)
MetadataLen = u8 byte_length(Metadata), 0..=127
Metadata = opaque user bytes
Count = u16 LE
MoveText = (EncodedMove, EncodedScore) * Count
```

- `MetadataLen` は 1 byte 固定です。v2 の metadata 上限は 127 bytes/chain なので、`0x80..=0xff` は不正値です。

- `MetadataLen=0` の場合、Metadata は空です。
- core v2 は Metadata の内部 schema を定義しません。rating、game id、engine name などを入れたい場合は、ユーザー側で bytes の内部構造を決めます。
- `MetadataLen` は `chunk_end` 内、かつ後続の `Count(u16)` を読める範囲に収まる必要があります。
- metadata は小さな sideband 用です。v2 の実装上限は 127 bytes/chain で、これを超える場合は不正データとして扱います。

### PackedSfenValue（38 bytes）

```text
packed_sfen[32]  # PackedSfen (互換)
score_i16        # Stemの評価値（cp, i16）
best_move_u16  # mv
ply_result_u16   # result(6bit) + ply(10bit, 局面のply)
```

## EncodedMove（v2）

```text
EncodedMove = ULEB128(legal_move_index)
```

- `legal_move_index` は **v1.0.0 オーダリング**のインデックス。
- 0 起点、最大 593。

v2 でも `EncodedMove` のオーダリングは v1.0.0 と同じです。

## Metadata（v2）

`Metadata` は sbinpack core から見て opaque な byte 列です。未使用時は `MetadataLen=0`、使用時は
`MetadataLen` で byte 数を宣言してから、そのまま `Metadata` を格納します。

固定 tag は v2 では予約しません。将来、rsshogi 標準 metadata schema を定義する場合も、この
opaque bytes の上に別仕様として載せます。

## EncodedScore（v2）

> 評価値は差分 + ZigZag + ULEB128 で可変長化する。

```text
norm_i = move_eval_i              # i が偶数
norm_i = -move_eval_i             # i が奇数
delta_0 = norm_0 - stem_score
delta_n = norm_n - norm_{n-1}     # n >= 1
EncodedScore = ULEB128(ZigZag(delta))
```

- `move_eval` は side-to-move 視点の cp（centipawn）です。
- 差分を取る前に、stem 側視点へ正規化します。現行の `MoveEntry` では `moves[i].eval` は「その手を指す前の局面」の評価値なので、`moves[0]` は stem と同一視点で反転しません。
- `stem_score` / 復元後の `move_eval` の意味は side-to-move のままです。正規化は wire 上の差分計算だけに使います。
- 評価値は ZigZag + ULEB128 で符号化された `i32` として扱います。`-32000..=32000` 外の値も特殊符号化せず、そのまま差分に乗せます。

### ZigZag 定義

```text
zigzag(i32) = (i32 << 1) ^ (i32 >> 31)
unzigzag(u32) = (u32 >> 1) ^ -(u32 & 1)
```

## ply_result の定義（v2）

```text
bit0..9:  ply (0..1023)
bit10..15: result (GameResult, 0..63)
```

- `result` は `GameResult` を 6bit で保持（現在の最大値は21）。
- `ply` は 10bit で保持（0..1023）。1023超は 1023 に丸める。

## v1.0.0 オーダリング（確定）

以下のキーを **この順番で**比較し、昇順が小さいインデックスになります。

1. **非打ち** → **打ち**
2. **quiet（非捕獲）** → **capture（捕獲）**
3. **非成り** → **成り**
4. **駒種優先順位**（下記）
5. **目的地(to)の段**（前進優先）
6. **目的地(to)の筋**（中央優先）
7. **移動元(from)の段**（前進優先）
8. **移動元(from)の筋**（中央優先）
9. **mv**（タイブレーク）

### 駒種優先順位（通常手）

```text
P > L > N > S > G > B > R > K > +P > +L > +N > +S > +B > +R
```

- 「素直な将棋の駒順」として説明可能性を優先。
- 成駒は基礎駒の後にまとめる。

### 駒種優先順位（打ち駒）

```text
P > L > N > S > G > B > R
```

### 段/筋の比較ルール

- **手番視点で正規化**してから比較する。
  - 先手: そのまま
  - 後手: 盤を 180 度回転（`inv`）
- **段**は前進優先（正規化後に `1 → 9`）。
- **筋**は中央優先（`5,4,6,3,7,2,8,1,9`）。

## オーダリングの根拠

- **安定性**: 統計で順序が揺れると互換性が崩れるため、ルールベースを採用。
- **自然な分類**: 打ち/非打ち、捕獲/非捕獲、成り/非成り、駒種、座標は将棋の標準的な分類。
- **圧縮効率**: 順序の大枠は頻度に沿う傾向があり、可変長符号化で十分な圧縮が得られる。
