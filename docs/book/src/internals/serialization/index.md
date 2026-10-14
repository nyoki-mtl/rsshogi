# シリアライズとビット圧縮

将棋エンジンおよび機械学習パイプラインでは、人間や USI プロトコルが読み書きできるテキスト表現（SFEN）と、数億〜数十億局面を高速かつ省メモリに記録するための固定長バイナリ表現（`PackedSfen` / `HuffmanCodedPos`）の双方が必要となる。
`rsshogi` は、中間構造体 `PositionState` を境界としてテキストパーサとバイナリ符号化器を分離している。

---

## シリアライズ表現の比較と Feature Gate

`rsshogi` がサポートする 3 種類の局面シリアライズ形式は以下の通りである。
バイナリ形式である `PackedSfen` と `HuffmanCodedPos` は、**`position-serialization` Cargo feature**（デフォルトで有効）によって提供される。

| 形式 | データ長 | 表現方式 | 主な用途 |
| :--- | :--- | :--- | :--- |
| **SFEN** | 可変長（約 40〜130 Bytes） | USI 標準の ASCII 文字列（盤面 + 手番 + 持ち駒 + 手数） | USI プロトコル通信、デバッグ出力、テストケース記述。 |
| **`PackedSfen`** | 固定 32 Bytes（256 bits） | YaneuraOu 互換のプレフィックス符号化バイナリ | YaneuraOu 系教師データ（`.bin` / `.binpack`）の入出力、定跡 DB。 |
| **`HuffmanCodedPos`** | 固定 32 Bytes（256 bits） | Apery / dlshogi 互換のハフマン符号化バイナリ | Apery / dlshogi 系教師データ（`HuffmanCodedPosAndEval`）の入出力。 |

---

## SFEN パーサとシリアライザ（`board/parser.rs`）

SFEN（Shogi Forsyth-Edwards Notation）文字列は、空白で区切られた 4 つのフィールド `<board> <side> <hands> <ply>` から構成される。

```text
lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

### パース処理のパイプライン

`Position::from_sfen(sfen)`（および `board/parser.rs`）は、ヒープアロケーションを伴わないイテレータ走査で以下の 4 段階の検証と構築を行う。

1. **盤面フィールド（`<board>`）の走査**：`'/'` で分割された 9 個の段（一段目 `rank = 0` から九段目 `rank = 8`）を順番に読み取り、各段の中では 9 筋（`file = 8`）から 1 筋（`file = 0`）へ向かって左から右へ走査する。
   - `'1'..='9'`：連続する空きマスの個数として `file` をデクリメントする。
   - `'+'`：直後の駒文字に対する成りフラグ（`promoted = true`）を立てる。玉や金に対して `'+'` が指定された場合や、行末で `'+'` が孤立した場合は `SfenError` を返す。
   - 駒文字（大文字は先手 `BLACK`、小文字は後手 `WHITE`）：対応する `Piece` を `Square::new(file, rank)` に配置する。各段の合計マス数がちょうど 9 マスでない場合や、段の総数が 9 段でない場合はエラーとする。
2. **手番フィールド（`<side>`）の解析**：`"b"` を `Color::BLACK`、`"w"` を `Color::WHITE` に変換する。
3. **持ち駒フィールド（`<hands>`）の解析**：`"-"` であれば両者の持ち駒を `Hand::EMPTY` とし、それ以外の場合はオプションの枚数プレフィックス（`2`〜`18`）と駒文字（`P, L, N, S, G, B, R` または小文字）のペアを順次読み取って、各駒種の物理上限枚数を超えていないか、および同じ駒種が重複指定されていないかを検証しながら `Hand` に格納する。
4. **手数フィールド（`<ply>`）の解析**：1 以上の 10 進整数（`u16`）としてパースし、省略時はデフォルトで `1` とする。

### シリアライズ処理（`to_sfen`）

`Position::to_sfen(&self)` は、一段目から九段目まで 9 筋 → 1 筋の順で盤面配列を走査し、連続する `Piece::NONE` をカウントして数字（`'1'..='9'`）に圧縮しながら文字列バッファへ書き出す。
持ち駒は USI 規約に従い、先手（大文字）→ 後手（小文字）の順、かつ各陣営内で **飛（R）→ 角（B）→ 金（G）→ 銀（S）→ 桂（N）→ 香（L）→ 歩（P）** の固定順序で出力される。

---

## `PositionState` による生状態境界の分離

SFEN パーサやバイナリデコーダ（`PackedSfen` / `HuffmanCodedPos`）は、最初から直接 `Position` の `BitboardSet` や `ZobristKey` を操作するのではなく、まず最小限の盤面、持ち駒、手番、手数のみを保持する中間構造体 `PositionState` を構築する。

```rust
pub struct PositionState {
    pub board: [Piece; Square::COUNT],
    pub hands: [Hand; Color::COUNT],
    pub side_to_move: Color,
    pub ply: u16,
}
```

この 2 段階構築には以下の設計上の利点がある。

1. **パーサと局面エンジンの責務分離**：各種フォーマットのデコーダは「ビット列や文字列を `board` と `hands` に展開すること」だけに専念でき、ビットボードの複合キャッシュ更新や Zobrist ハッシュ計算、ピンや王手マスの初期化といった複雑なロジックはすべて `Position::from_raw_state(state)` の 1 箇所に集約される。
2. **外部入力の安全な検証境界**：外部の破損したバイナリファイルや不正な SFEN から構築された `PositionState` に対し、`Position` へ変換する境界で玉の位置や持ち駒の不変条件を一元的に検査できる。

次ページでは、`PositionState` と 256 ビット固定長バイナリを相互変換する [ビット圧縮アルゴリズム（`PackedSfen` / `HuffmanCodedPos`）](compression.md) の詳細を解説する。
