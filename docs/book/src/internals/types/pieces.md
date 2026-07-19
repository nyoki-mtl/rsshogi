# 駒の定義

> **前提知識**: [座標系の定義](./coordinates.md)

## このページの要点

- 駒は **PieceType**（駒種: 0〜15）と **Piece**（駒＋先後: 0〜31）の 2 階層で表現する
- 成り変換は **+8 オフセット**（bit 3 を立てるだけ）で実現し、条件分岐が不要
- 先後の判定は **bit 4** の検査で 1 命令で完了する
- このインデックス体系は 互換であり、デバッグ時に他エンジンの値と直接比較できる

## なぜ 2 階層か

将棋では「駒の種類だけが分かればよい場面」と「誰の駒かも分かる必要がある場面」が明確に分かれています。

- **持ち駒**: 先手の持ち駒に「歩」が3枚。ここでは `PieceType` だけで十分（先手であることは文脈で分かる）
- **盤上の駒**: 5五に先手の飛車がある。ここでは `Piece`（`B_ROOK`）が必要

1 つの型で両方をカバーしようとすると、持ち駒のカウントで余分な先後情報を持つか、盤上の駒で先後判定に追加コストが掛かります。
2 階層に分離することで、それぞれの場面で最小限の情報だけを扱えます。

rsshogi における駒の内部表現を解説します。
この番号体系は 参照実装の定義を参考にしています。[^yaneuraou-piece]

## 2階層の駒表現

駒は `PieceType`（駒種）と `Piece`（駒＋先後）の 2 階層で表現されます。

### PieceType（駒種）

駒種は 0〜15 のインデックスで表現されます：

| インデックス | 定数名 | 駒種 |
|------------|--------|------|
| 0 | `NONE` | 空 |
| 1 | `PAWN` | 歩 |
| 2 | `LANCE` | 香 |
| 3 | `KNIGHT` | 桂 |
| 4 | `SILVER` | 銀 |
| 5 | `BISHOP` | 角 |
| 6 | `ROOK` | 飛車 |
| 7 | `GOLD` | 金 |
| 8 | `KING` | 玉 |
| 9 | `PRO_PAWN` | と金 |
| 10 | `PRO_LANCE` | 成香 |
| 11 | `PRO_KNIGHT` | 成桂 |
| 12 | `PRO_SILVER` | 成銀 |
| 13 | `HORSE` | 馬 |
| 14 | `DRAGON` | 龍 |
| 15 | `GOLD_LIKE` | 金相当（金・と金・成香・成桂・成銀をまとめて扱う内部補助種） |

**成り変換**: 成駒は元の駒に +8 を加えたインデックスです：

```rust,ignore
piece_type.promote() = PieceType(piece_type.0 + 8)
```

#### 実装: PieceType 構造体

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_type_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L31-L36)</small>

#### 実装: 駒種定数

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_type_constants}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L39-L86)</small>

#### 実装: 成り変換メソッド

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_type_promote}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L95-L132)</small>

### Piece（駒＋先後）

先後の区別を含む駒は 0〜31 のインデックスで表現されます：

| インデックス範囲 | 定数プレフィックス | 説明 |
|----------------|------------------|------|
| 0 | `NO_PIECE` | 空 |
| 1〜15 | `B_*` | 先手の駒（例: `B_PAWN` = 1） |
| 17〜31 | `W_*` | 後手の駒（例: `W_PAWN` = 17） |

**先後フラグ**: 後手の駒は先手の駒に +16 を加えたインデックスです：

```rust,ignore
piece = Piece(piece_type.0 + color.0 * 16)
```

#### 実装: Piece 構造体

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_struct}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L253-L258)</small>

#### 実装: 駒定数

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_constants}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L261-L342)</small>

## エンコーディングの構造

Piece のエンコーディングはビットフラグで整理できます：

Piece = [先後フラグ(bit 4)] | [駒種(bit 0-3)]
                                  └ bit 3 が成りフラグ（生駒 +8 で立つ）

```text
例:
B_PAWN   = 0b00001 = 1   (先手・歩)
B_HORSE  = 0b01101 = 13  (先手・馬 = 成角)
W_PAWN   = 0b10001 = 17  (後手・歩)
W_DRAGON = 0b11110 = 30  (後手・龍 = 成飛)
```

駒種は **bit 0-3 の 4 ビット**（`Piece::piece_type()` は `self.0 & 15` で取得）、先後は **bit 4**（`& 0b10000`）です。
成りフラグは独立したビットではなく、4 ビットの駒種フィールドのうち bit 3（= +8）が成りを表します。

この構造により、以下のアクセサが簡潔になります：

#### 実装: 駒アクセサ

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/piece.rs:piece_accessors}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/piece.rs#L351-L378)</small>

## ビット操作による最適化

### なぜ金を飛車の後ろに配置するのか

駒種のインデックス配置は、成り処理の効率化を考慮して設計されています：

```text
1: PAWN   →  9: PRO_PAWN    (+8)
2: LANCE  → 10: PRO_LANCE   (+8)
3: KNIGHT → 11: PRO_KNIGHT  (+8)
4: SILVER → 12: PRO_SILVER  (+8)
5: BISHOP → 13: HORSE       (+8)
6: ROOK   → 14: DRAGON      (+8)
7: GOLD   (成れない)
8: KING   (成れない)
```

この配置により、成り処理が単純なビット演算で実現できます：

```rust,ignore
// 成る: bit 3 を立てる（+8）
promoted_piece = piece | 8;

// 元の駒に戻す（持ち駒変換用）: bit 3 を落とす
demoted_piece = piece & !8;
```

もし金が歩の直後（インデックス2）に配置されていた場合、成り判定で複雑な条件分岐が必要になります。
金を後ろに配置することで、「成れる駒（1-6）」と「成れない駒（7-8）」の境界が明確になります。

### KINGを8にする理由

玉を8番目に配置する理由も、ビット操作による効率化です：

```rust,ignore
// 玉かどうかの判定
if piece_type == 8 {
    // 玉の処理
}

// より効率的: bit 3 だけをチェック
if (piece_type & 8) != 0 && piece_type < 9 {
    // 成れない駒（金=7または玉=8）
}
```

この設計により、多くの条件判定がビット演算で高速に実行できます。

## USI 表記

USI プロトコルでは駒を以下の文字で表現します：

| PieceType | USI表記 | 成駒のUSI表記 |
|-----------|--------|--------------|
| PAWN | `P` | `+P` |
| LANCE | `L` | `+L` |
| KNIGHT | `N` | `+N` |
| SILVER | `S` | `+S` |
| BISHOP | `B` | `+B` |
| ROOK | `R` | `+R` |
| GOLD | `G` | - |
| KING | `K` | - |

先後は盤面表記では大文字/小文字で区別しますが、持ち駒では区別しません。

## 将棋特有の考慮事項

チェスでは駒種が 6 種類（Pawn, Knight, Bishop, Rook, Queen, King）で、成りは Pawn のみがクイーンなど別の駒に変身（promotion）します。
将棋では以下の点が大きく異なります。

| 項目 | チェス | 将棋 |
|------|--------|------|
| 駒種数 | 6 | 14（生駒 8 + 成駒 6） |
| 成り | Pawn のみ → 別駒種に変身 | 金・玉以外の6種 → 元の駒種 +8 |
| 持ち駒 | なし | あり → PieceType だけで管理 |
| 先後の駒配列サイズ | 12（6×2） | 32（16×2、うち index 16 は未使用） |

この複雑さが 2 階層構造と `+8` 成りオフセットの設計を導いています。

## 落とし穴

### `NO_PIECE` (0) と `B_PAWN` (1) の境界

`board[sq]` を初期化し忘れると値が 0 になり、`NO_PIECE`（空マス）と解釈されます。
逆に `board[sq] == NO_PIECE` のチェックを忘れて駒の種類を取得すると、不正な `PieceType(0)` が返ります。

### 成駒の demote 忘れ

駒を取って持ち駒に加える際、成駒を元の駒種に戻す（demote）必要があります。

```rust,ignore
// ❌ 間違い: 成銀を持ち駒に加えてしまう
hand.add(captured.piece_type());  // PRO_SILVER (12) が持ち駒に入る

// ✅ 正しい: 生駒に戻してから加える
hand.add(captured.piece_type().demote());  // SILVER (4) が持ち駒に入る
```

### Piece のインデックス 16 は未使用

Piece のインデックス空間は 0〜31 ですが、16 は使用されていません（`W_PAWN` は 17）。
配列のサイズを `Piece::NUM`（= 32）で確保すれば問題ありませんが、
「後手の駒は先手 +16」というルールから `W_NO_PIECE = 16` が存在しうる点に注意してください。

## まとめ

- **PieceType**: 駒種（0〜15）。持ち駒の管理やインデックスに使用
- **Piece**: 駒種＋先後（0〜31）。盤面の各マスに格納
- **成り変換**: `+8`（bit 3 を立てる）だけで完了。分岐不要
- **先後判定**: bit 4 のチェック 1 回で完了
- インデックス設計は 互換で、既存エンジンの知見をそのまま活用可能

## 次に読む

→ **[指し手の表現](./moves.md)**: 16 ビットに凝縮された指し手のエンコーディングと、その設計上のトレードオフを解説します。

---

[^yaneuraou-piece]: YaneuraOu `types.h:219-287`（[Commit eb2856f](https://github.com/yaneurao/YaneuraOu/blob/eb2856f91e6088e5b8c1216e3d84d0563f7cd85f/source/types.h#L219-L287)）
