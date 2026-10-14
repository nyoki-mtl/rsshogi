# プリミティブ型のビットレイアウト

将棋エンジンおよび棋譜処理ライブラリの性能は、最下層のプリミティブ型がいかにコンパクトかつ CPU 命令に親和性の高い表現を持つかに左右される。
`rsshogi` では、座標、駒、持ち駒、指し手、評価値などの基本型をすべて 1〜4 バイトの整数を包む Newtype（`#[repr(transparent)]`）または固定幅列挙型として定義している。

---

## プリミティブ型のサイズと内部表現一覧

主要プリミティブ型のメモリサイズ、内部表現、および有効値の範囲を以下に示す。

| 型名 | サイズ | 内部表現 | 有効値の範囲とセンチネル | 主な用途 |
| :--- | :--- | :--- | :--- | :--- |
| `Color` | 1 Byte | `u8` | `0..=1`（`BLACK = 0`、`WHITE = 1`） | 手番と陣営の識別。 |
| `File` | 1 Byte | `i8` | `0..=8`（1 筋〜9 筋） | 筋の表現とインデックス変換。 |
| `Rank` | 1 Byte | `i8` | `0..=8`（一〜九段） | 段の表現と成段判定。 |
| `Square` | 1 Byte | `i8` | `0..=80`、センチネル `SQ_NONE = 81` | 盤面 81 マスの一次元座標。 |
| `PieceType` | 1 Byte | `i8` | `0..=15`（`NONE = 0`、実駒 `1..=14`、`GOLD_LIKE = 15`） | 先後を区別しない駒種。 |
| `Piece` | 1 Byte | `i8` | `0..=31`（先手 `1..=15`、後手 `17..=31`、`16` は無効） | 先後と成り状態を含む盤上の駒。 |
| `HandPiece` | 1 Byte | `i8` | `0..=6`（歩、香、桂、銀、角、飛、金） | 持ち駒テーブルの 7 種インデックス。 |
| `Hand` | 4 Bytes | `u32` | 各駒種の枚数ビットとボロービットをパックした 32 ビット値 | 片方の陣営の全持ち駒枚数。 |
| `Move` | 2 Bytes | `u16` | 移動先 7b、移動元/打駒 7b、打フラグ 1b、成フラグ 1b | USI / YaneuraOu 互換の 16 ビット指し手。 |
| `Move32` | 4 Bytes | `u32` | 下位 16b `Move` + ビット `16..=20` に移動後の `Piece` | 盤面更新高速化およびオーダリング用の 32 ビット指し手。 |
| `AperyMove` | 2 Bytes | `u16` | 移動先 7b、移動元/打駒（`81..=87`）7b、成フラグ 1b | Apery（WCSC26）互換の 16 ビット指し手。 |
| `AperyMove32` | 4 Bytes | `u32` | 下位 16b `AperyMove` + 移動前 `PieceType` 4b + 捕獲 `PieceType` 4b | Apery 互換の 32 ビット指し手。 |
| `Ki2Notation` | 4 Bytes | `u32` | 手番 1b、同フラグ 1b、移動先 7b、駒種 4b、相対位置 4b、成不成 2b | KI2 形式の指し手表記情報。 |
| `MoveLabel` | 2 Bytes | `u16` | `0..=2186`（`class * 81 + to_sq`） | DL 将棋用の 2187 クラス方策ラベル（`policy-labels` feature）。 |
| `CompactMoveLabel` | 2 Bytes | `u16` | `0..=1495` | 構造的不可能手を除外した 1496 クラス圧縮ラベル（`policy-labels` feature）。 |
| `Eval` | 4 Bytes | `enum`（`i16` ペイロード） | `Cp(-32000..=32000)` または `Special(i16)` | 評価値と詰みスコアの表現。 |
| `GameResult` | 1 Byte | `#[repr(u8)]` | `0..=2`（`BlackWin = 0`、`WhiteWin = 1`、`Draw = 2`） | 対局結果の表現。 |
| `RepetitionState` | 1 Byte | `enum` | `None`、`Win`、`Lose`、`Draw`、`Superior`、`Inferior` | 千日手、連続王手の千日手、優等局面と劣等局面の判定結果。 |
| `EnteringKingRule` | 1 Byte | `#[repr(u8)]` | `NoEnteringKing`、`Cs24Point`、`Cs27Point`、`Cs27PointHandicap`、`TryRule` | 入玉宣言勝ちのルール種別。 |

---

## 型安全とゼロコスト抽象化の境界

`rsshogi` のプリミティブ型は、公開 API における型安全性と内部ホットパスにおける配列アクセス速度を両立するため、以下の 3 層で不変条件（Invariant）を管理している。

1. **安全コンストラクタ（`from_raw` / `new`）**：外部入力やシリアライズデータから型を構築する際は、有効範囲を検査して `Option<T>` または `Result<T, E>` を返す。
2. **信頼済み境界（`from_raw_unchecked`）**：`pub(crate)` または `unsafe` 関数として提供され、呼び出し側が有効範囲内であることを保証する内部ループでのみ使用される。`debug_assert!` によってデバッグビルド時に不変条件違反を即座に検出する。
3. **型付きテーブル（`SquareTable<T, N>` など）**：`Square` や `PieceType` を直接インデックスとして受け取るラッパー型を用意し、`to_index()` による境界チェックの重複を省きつつ、生整数（`usize`）の取り違えをコンパイル時に防止している。

---

## 各サブセクションの内容

- **[座標系（Color / File / Rank / Square）](coordinates.md)**：File-major 座標計算、除算と剰余を排除する 256 要素ルックアップテーブル（`RAW_TO_FILE` / `RAW_TO_RANK`）、および `SquareTable` の仕組みを解説する。
- **[駒と持ち駒（PieceType / Piece / Hand）](pieces.md)**：`Piece` のビット構造と `Hand(u32)` の 7 駒種パッキングおよびボロービットによる一括優劣比較アルゴリズムを解説する。
- **[指し手表現（Move / Move32 / AperyMove / Ki2Notation）](moves.md)**：USI 系と Apery 系のビットレイアウト比較、特殊指し手定数の衝突回避、`complete_move32` と `apply_move32` の検証仕様、および `Ki2Notation` の 32 ビット構造を解説する。
- **[方策ラベル（MoveLabel / CompactMoveLabel）](policy-labels.md)**：2187 ラベルの算術的定義と、コンパイル時生成テーブルによる 1496 クラスへの全単射圧縮を解説する。
