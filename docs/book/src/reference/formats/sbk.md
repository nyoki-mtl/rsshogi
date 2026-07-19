# SBK 形式

SBK（拡張子 `.sbk`）は **ShogiGUI** 由来の定跡フォーマットです。
局面をキーに、定跡手の一覧・各手の評価・出現頻度・コメントなどを格納します。
プロ棋士を含む将棋関係者に広く使われており、ShogiHome は v1.28.0 以降で対応しています。

rsshogi は読み取り専用リーダ [`SbkBook`](external-books.md#sbk) と
`BookDatabase` からの full export writer を提供します。本ページはフォーマットそのものの
構造と、rsshogi がそれをどう解釈・生成するかを説明します。

> 出典・背景: 久保 良輔 氏（[@sunfish-shogi](https://qiita.com/sunfish-shogi)）による解説記事
> [「将棋の定跡ファイル形式 SBK について」](https://qiita.com/sunfish-shogi/items/f47eafe6ca36d086e578)。
> スキーマのフィールド番号・型は rsshogi の hand-written リーダ
> （`crates/rsshogi/src/book/sbk.rs`）が実際に解釈するワイヤ表現に基づきます。

## 設計の特徴

- **Protocol Buffers ベース**: コンパクトかつ高速にデコードできるバイナリ表現。
  冗長な情報を省くことでファイルサイズを比較的小さく抑えます。
- **状態グラフ構造**: 局面（state）を node、定跡手（move）の `nextStateId` を edge と
  するグラフ。1 つの局面から指し手をたどって次の局面へ遷移できます。
- **バイナリサーチ非前提**: DB2016 や Apery BIN がソート済みで二分探索を
  想定するのに対し、SBK は **そのままでは局面検索に向きません**。素朴に扱うなら
  デコード結果をメモリへ展開してインデックスを作る必要があります
  （rsshogi の対処は[後述](#rsshogi-のリーダ実装)）。

## Protobuf スキーマ

rsshogi のリーダが解釈するメッセージは 4 つです。フィールド番号は wire 上のタグ、
型は protobuf 上の表現です。

```proto
// 定跡ファイル全体
message SBook {
  optional string author      = 1;  // 作成者
  optional string description = 2;  // 説明
  repeated SBookState state   = 3;  // 登録局面（state）の列
}

// 1 つの登録局面
message SBookState {
  optional int32  id        = 1;   // state の論理 ID（物理インデックスとは別物）
  // フィールド 2, 3: BoardKey / HandKey（後述。rsshogi は読まない）
  optional int32  games     = 4;   // この局面の出現対局数
  optional int32  wonBlack  = 5;   // 先手勝ち数
  optional int32  wonWhite  = 6;   // 後手勝ち数
  optional string position  = 7;   // 局面の SFEN
  optional string comment   = 8;   // コメント
  repeated SBookMove moves  = 9;   // 候補手
  repeated SBookEval evals  = 10;  // 評価・探索情報
}

// 1 つの定跡手
message SBookMove {
  optional int32 move        = 1;  // 指し手（数値エンコード）
  optional int32 evaluation  = 2;  // 指し手評価 enum（0=None, 1=Forced, ...）
  optional int32 weight      = 3;  // 重み（出現頻度など）
  optional int32 nextStateId = 4;  // 遷移先 state の id（-1 / 未設定で終端）
}

// 1 つの評価・探索情報レコード
message SBookEval {
  optional int32  value      = 1;  // 評価値
  optional int32  depth      = 2;  // 探索深さ
  optional int32  selDepth   = 3;  // 選択的探索深さ
  optional int64  nodes      = 4;  // 探索ノード数（64bit）
  optional string variation  = 5;  // 読み筋
  optional string engineName = 6;  // エンジン名
}
```

> このスキーマは rsshogi のリーダが解釈する範囲を示すものです。フィールド名や
> 未使用フィールドの正確な定義は、変換ツール **BookConv**（ShogiGUI 開発者公開）の
> `book.proto` が一次情報です。

## 局面のエンコード（BoardKey / HandKey の扱い）

SBK の `SBookState` は局面を **SFEN 文字列**（`position`, フィールド 7）として持ち、
加えて `BoardKey` / `HandKey`（フィールド 2 / 3）を格納します。ただし、

- これらキーの**計算ロジックは非公開**で、BookConv にも含まれていません。
  ShogiGUI は SFEN から独自に算出していると見られます。

そのため rsshogi は **`BoardKey` / `HandKey` を読まず**、`position` の SFEN を
`Position::from_sfen()` でパースし直し、自前の [Packed SFEN](external-books.md#packed-sfen)
（`Position::to_packed_sfen()`）をキーとして使います。これにより、非公開キーの
互換性に依存せず一貫した局面同一性で検索できます。

## state グラフと nextStateId

各 `SBookMove` の `nextStateId` は、その手を指した後の局面に対応する
`SBookState::id` を指します。これにより 1 局面の候補手から次局面の state を
たどれます。`nextStateId` が負・未設定、または解決できない id の場合は終端
（子局面なし）として扱います。

rsshogi では `SbkBook::child_entry(&parent, move_index)` がこのリンクをたどります。

```rust,ignore
let parent = book.lookup_sfen(sfen)?.expect("parent entry");
if let Some(child) = book.child_entry(&parent, 0)? {
    println!("{}", child.sfen());
}
```

なお、`SBookState::id` は**論理 ID** であり物理的な格納順とは一致しません。
rsshogi は両者を区別し、`lookup_state_id()` は `id` で、`lookup_state_index()` は
物理 state インデックスで解決します。

## rsshogi のリーダ実装

SBK はそのままでは二分探索に向かないフォーマットですが、rsshogi は
**全グラフをメモリ展開せずに**検索可能にします。

1. **オープン時**: protobuf の state オフセットだけを走査し、各 state を潜在的な
   ルートとしてたどって、`position` SFEN または `nextStateId` リンクから Packed SFEN
   キーを導出。これを**コンパクトなソート済みインデックス**（state 数に比例）へ格納。
2. **検索時**: インデックスを Packed SFEN の SBK ワード順で二分探索し、一致した
   **state payload だけ**をオンデマンドにデコード。

不正なグラフ辺はスキップされるため、壊れた分岐があっても定跡の残りは開けます。
`SbkBook::diagnostics()` は重複局面と未解決 payload を報告します。

> ここで二分探索の対象になるのは rsshogi がオープン時に構築する**メモリ上の
> 派生インデックス**であって、ファイルのバイト列ではありません。SBK のオンディスク
> 表現は依然ソート済みではなく、これがフォーマット本来の「メモリ展開が必要」という
> 性質への rsshogi の対処になっています。

API の使い方は [外部定跡（DB2016 / YBB / SBK）](external-books.md#sbk) を参照してください。

## rsshogi の writer 実装

SBK writer は [`BookDatabase`](book-architecture.md#編集-ir-層--bookdatabase) 全体を
新しい protobuf として再生成する full export です。

- state ID は `BookStates[i].Id == i` の連番に正規化します。
- 出力 state は root を先に並べます。入力 `BookDatabase` の順序が child → root でも、
  `NextStateId` は出力後の state ID へ再マップします。
- `NextStateId` は、候補手を合法に適用して得た子 SFEN が `BookDatabase` に存在するかで
  再構築します。子局面がない手は `-1` とし、空の leaf state は合成しません。
- `SBookMove.Evaluation` は数値評価ではなく BookConv / ShogiGUI の enum として扱い、
  SBK 由来 metadata がある場合だけそれを保持します。未指定の候補手は `0` を出力します。
- `SBookEval` の探索情報、state の games / 勝敗数 / comment、move の weight は
  SBK metadata として保持されている範囲で出力します。
- top-level の author / description は現行 `BookDatabase` に格納先がないため出力しません。
- `BoardKey` / `HandKey` は非公開計算に依存しないため 0 として出力します。

on-the-fly の差分 patch merge writer は提供しません。

## 関連ツール

- **BookConv**: ShogiGUI 開発者が公開する SBK 変換ツール。`book.proto` の一次情報。
- **ShogiHome**: 久保氏開発の将棋ソフト。v1.28.0 以降で SBK に対応。

## 関連項目

- [定跡アーキテクチャ（3 層モデル）](book-architecture.md)：SBK リーダがどの層に
  位置するか
- [外部定跡（DB2016 / YBB / SBK）](external-books.md)：`SbkBook` と writer の Rust API
- [DB2016 形式](yaneuraou.md)：もう一つの外部定跡フォーマット
- [外部定跡（Python）](../../python/external-books.md)：Python からの利用
- [定跡バイナリ](book.md)：rsshogi 独自の `StaticBook` 形式
