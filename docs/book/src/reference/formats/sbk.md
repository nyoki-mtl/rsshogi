# ShogiHome SBK フォーマット仕様

SBK（拡張子 `.sbk`）は、ShogiGUI 由来の定跡バイナリフォーマットであり、ShogiHome（v1.28.0 以降）や変換ツール BookConv で広く利用されている。
Protocol Buffers（Protobuf）をベースとし、局面（state）を頂点、定跡手（move）の遷移先 `nextStateId` を有向辺とする状態グラフ構造を持つ。

`rsshogi` は読み取り専用リーダ `SbkBook`（Rust および Python）と、編集 IR である `BookDatabase` からの全体書き出し API（Rust）を提供する。

## 1. Protobuf ワイヤースキーマ

`rsshogi` の SBK リーダおよびライタが扱う Protobuf メッセージ定義は以下の通りである。

```proto
// 定跡ファイル全体
message SBook {
  optional string author      = 1;  // 作成者名
  optional string description = 2;  // 定跡の説明文
  repeated SBookState state   = 3;  // 登録局面（state）の配列
}

// 1 つの登録局面（頂点）
message SBookState {
  optional int32  id        = 1;   // state の論理 ID（物理配列インデックスとは独立）
  optional uint64 boardKey  = 2;   // ShogiGUI 固有の盤面キー（rsshogi 読込時は無視、書出時は 0）
  optional uint32 handKey   = 3;   // ShogiGUI 固有の持駒キー（rsshogi 読込時は無視、書出時は 0）
  optional int32  games     = 4;   // この局面の出現対局数
  optional int32  wonBlack  = 5;   // 先手勝ち数
  optional int32  wonWhite  = 6;   // 後手勝ち数
  optional string position  = 7;   // 局面の SFEN 文字列
  optional string comment   = 8;   // 局面コメント
  repeated SBookMove moves  = 9;   // 候補手リスト
  repeated SBookEval evals  = 10;  // エンジン評価および探索情報リスト
}

// 1 つの候補手（有向辺）
message SBookMove {
  optional int32 move        = 1;  // 指し手（数値エンコード）
  optional int32 evaluation  = 2;  // 指し手評価シンボル enum（0=None, 1=Forced 等）
  optional int32 weight      = 3;  // 重み（採択回数や頻度）
  optional int32 nextStateId = 4;  // 遷移先 SBookState の論理 id（-1 または未設定で終端）
}

// 1 つのエンジン評価レコード
message SBookEval {
  optional int32  value      = 1;  // 評価値（cp）
  optional int32  depth      = 2;  // 探索深さ
  optional int32  selDepth   = 3;  // 選択的探索深さ
  optional int64  nodes      = 4;  // 探索ノード数
  optional string variation  = 5;  // 読み筋（PV 文字列）
  optional string engineName = 6;  // エンジン名
}
```

## 2. オンデマンド検索と `state_id` グラフ解決の仕組み

SBK ファイル内の `SBookState` は必ずしも特定の順序で並んでいるとは限らず、また一部の state では `position`（SFEN）が省略され、親 state の候補手（`nextStateId`）からの遷移によってのみ局面が定義される場合がある。
`SbkBook` はオープン時に以下のインデックス構築を行い、検索時の省メモリかつ高速なオンデマンドデコードを実現している。

### オープン時のインデックス構築

1. **オフセットと論理 ID マップの構築**：Protobuf ストリームを走査して各 `SBookState` のファイル内バイトオフセット（物理インデックス `state_index: 0..N`）を記録し、論理 ID（`SBookState.id`）から物理インデックスへの変換マップ（`state_id -> state_index`）を構築する。
2. **局面の導出と `PackedSfen` 索引の生成**：各 state の明示的な `position`（SFEN）または親 state からの `nextStateId` 辺を辿って各 state の 32 バイト `PackedSfen` を導出し、SBK ワード順（`PackedSfen::cmp_sbk_words`）でソートしたコンパクトな検索インデックスを作成する。不正なグラフ辺が存在してもその分岐のみをスキップし、定跡全体のオープンは継続される。
3. **オンデマンドデコード**：`lookup_sfen` や `lookup_position` が呼ばれた際は、メモリ上の `PackedSfen` インデックスを二分探索し、一致した `state_index` の Protobuf ペイロードだけをその場でデコードして `SbkEntry` を返す。

### `next_state_id`（論理 ID）と `state_index`（物理インデックス）の違い

`SBookMove.nextStateId`（`SbkMove::next_state_id()`）が保持する値は、遷移先 `SBookState` の **論理 ID（`id` フィールド）** であり、`SBook.state` 配列の物理インデックス（`0..N`）ではない。
`SbkBook` では、この 2 種類の識別子を明確に区別して解決するメソッドを提供している。

| メソッド | 引数の意味 | 動作 |
| :--- | :--- | :--- |
| `lookup_state_id(state_id: i32)` | `SBookState.id`（論理 ID） | オープン時に構築した `state_id -> state_index` マップを引いて該当 state をデコードする。負の値や未登録 ID には `Ok(None)` を返す。 |
| `lookup_state_index(state_index: usize)` | `SBook.state` の配列添字（物理インデックス） | `state_index` 番目の state ペイロードを直接デコードする。範囲外には `Ok(None)` を返す。 |
| `child_entry(&parent, move_index)` | 親 `SbkEntry` と候補手添字 | `parent.moves()[move_index].next_state_id()` を取得し、内部で `lookup_state_id` を呼んで遷移先の子 `SbkEntry` を返す。終端手（`-1` または `None`）の場合は `Ok(None)` を返す。 |

## 3. `BookDatabase` からの SBK 書き出し

Rust の `BookDatabase::write_sbk` および `to_sbk_bytes`（`SbkWriteOptions`）は、データベース内の全局面と候補手から新しい SBK Protobuf バイナリを再生成する。

- **論理 ID の正規化**：ルート局面を先頭に配置したうえで、出力される各 `SBookState` に対して `state[i].id == i as i32` の連番を割り当てる。
- **`nextStateId` の自動再構築**：各候補手 `mv` を現在の局面で合法的に 1 手進めた後の正規化 SFEN を計算し、その子局面が `BookDatabase` 内に存在する場合は対応する新しい `state_id` を `nextStateId` に設定する。存在しない場合（終端）は `-1` を出力する。
- **`BoardKey` と `HandKey`**：ShogiHome 互換の `0` を出力する。
- **メタデータの保持**：`SbkEntryMetadata` および `SbkMoveMetadata` に保持されている `games`、`won_black`、`won_white`、`comment`、`evals`（`SbkEval`）、`evaluation`（指し手評価 enum）、`weight` をそのまま出力する。

## 4. コード例

### Python

```python
from rsshogi import Board, SbkBook

# SBK 定跡を開いて初期局面を検索する
book = SbkBook.open("opening.sbk")
print("author:", book.author, "states:", len(book))

board = Board()
entry = book.lookup_position(board)
if entry is not None:
    for i, mv in enumerate(entry.moves):
        print(mv.mv.to_usi(), "weight:", mv.weight, "next_id:", mv.next_state_id)
        # 遷移先の子局面エントリを辿る
        child = book.child_entry(entry, i)
        if child is not None:
            print("  -> child sfen:", child.sfen)
```

### Rust

```rust
use rsshogi::board;
use rsshogi::book::{BookControl, BookDatabase, SbkBook, SbkWriteOptions};

board::init();
let pos = board::hirate_position();

// 進捗コールバック付きで SBK ファイルを開く
let book = SbkBook::open_with_control("opening.sbk", |progress| {
    let _ = (progress.indexed_states(), progress.total_states());
    BookControl::Continue
})?;

// 局面検索と状態グラフの追跡
if let Some(parent) = book.lookup_position(&pos)? {
    if let Some(child) = book.child_entry(&parent, 0)? {
        println!("Child SFEN: {}", child.sfen());
    }
}

// BookDatabase へ取り込んで SBK バイナリとして書き出す
let mut db = BookDatabase::new();
for entry_res in book.iter_entries() {
    let entry = entry_res?;
    db.upsert_entry(BookDatabase::entry_from_sbk(&entry)?)?;
}
let sbk_bytes = db.to_sbk_bytes(&SbkWriteOptions::new())?;
```

## 関連項目

- [定跡 3 層アーキテクチャ](book-architecture.md)
- [YaneuraOu 定跡フォーマット仕様（DB2016 / YBB）](yaneuraou.md)
- [StaticBook バイナリフォーマット仕様](book.md)
