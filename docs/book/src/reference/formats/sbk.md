# ShogiHome SBK フォーマット仕様

SBK（拡張子 `.sbk`）は、ShogiGUI 由来の定跡バイナリフォーマットであり、ShogiHome（v1.28.0 以降）や変換ツール BookConv で広く利用されている。
Protocol Buffers（Protobuf）をベースとし、局面（state）を頂点、定跡手（move）の遷移先 `nextStateId` を有向辺とする状態グラフ構造を持つ。

`rsshogi` は読み取り専用リーダ `SbkBook`（Rust および Python）と、編集 IR である `BookDatabase` からの全体書き出し API（Rust）を提供する。
1.7.1 では、既存 reader と変更局面だけの `BookDatabasePatch` から逐次出力する Rust API も追加する。
IR からの再生成と既存ファイルの差分保存は、ID とリンク、未知フィールドの扱いが異なる。

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
  optional uint64 boardKey  = 2;   // ShogiGUI 固有の盤面キー（局面解決には使わない）
  optional uint32 handKey   = 3;   // ShogiGUI 固有の持駒キー（局面解決には使わない）
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

全ファイルのバイト列を RAM に置かず、逐次走査と state 単位の読込を行う。
索引構築のグラフ走査には本文上限 4 MiB のページキャッシュを使う。
局面索引と queue は state 数に比例し、キャッシュの管理情報と最大 state の読込バッファ、著者と説明の文字列も必要になる。
全ファイルを RAM に保持する方式より読込時間が増える場合があり、一定メモリや一定時間での open は保証しない。

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
from rsshogi.book import SbkBook
from rsshogi.core import Board

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

## 5. 既存 SBK の差分保存

`SbkBook::write_sbk_with_patch` は `BookDatabasePatch` の局面置換と削除を反映し、元の物理順を保って出力する。
新規局面は正規化 SFEN 順で末尾に追加し、ID は `state[i].id == i` へ振り直す。
全局面の `Position` と全候補手と出力全文は保持せず、索引と最大 state と差分に比例するメモリを使用する。
open 時もファイル全体のバイト列を保持せず、バッファ付きの入力から局面索引を構築する。
索引構築時は最大 4 MiB のページ本文キャッシュを使う。
局面索引、キャッシュの管理情報、最大 state のバッファは別途必要になる。

既存の候補手は、コメントや評価の変更後も元のリンクを保つ。
削除された子へのリンクは `-1` とし、候補手自体は残す。
新規候補手は合法手適用で子局面を求め、出力局面集合にあれば接続する。
既存の終端の先に局面を追加しただけでは自動接続しない。
元の明示 SFEN を保持し、出力の先頭と復元起点が不足する箇所、および明示的な手数指定のある局面に SFEN を補完する。
親の削除で孤立した局面や循環成分も保存する。

著者、説明、未変更の統計、評価一覧、未知 wire フィールドを保持する。
置換対象の state は、コメントだけの変更でも `boardKey` と `handKey` を 0 で出力する。
未置換の state は元の値を保持し、これらのキーは局面解決には使用しない。
置換局面でも state の未知フィールドと、同じ指し手に残る未知フィールドを保持する。
未知フィールド付き評価一覧は、既知の値を変更せずそのまま残せる場合だけ置換できる。
未知フィールド内の将来の ID 参照の意味までは保証しない。

重複局面、未解決 state、不正な指し手、矛盾する辺、欠けた非終端参照を持つ入力は保存を拒否する。
これは既存 reader の寛容な受理範囲とは異なる。
SBK に表現できない ponder、depth、count、DB2016 metadata と、指し手評価 metadata に対応しない score は `Unsupported` とする。
明示手数は reader が扱う `u16` の範囲を要求する。
open 時に未訪問の子への着手で手数がこの範囲を超える場合は、子に明示 SFEN があっても `InvalidData` を返す。
さらに、保存後のグラフ走査で手数加算がその範囲を超える場合も出力前に拒否する。
全局面削除は空 SBK を reader が拒否するため `InvalidData` とする。

`write_sbk_with_patch_with_control` は `Validating` と `Writing` の段階を通知し、`BookControl::Cancel` で中断する。
件数は元 state の処理数であり、準備処理や追加局面の出力中は据え置かれるため、全体の完了率や残り時間には使えない。
出力失敗時も差分を保持する。
出力の永続化と置換は[定跡アーキテクチャ](book-architecture.md)を参照する。

## 関連項目

- [定跡 3 層アーキテクチャ](book-architecture.md)
- [YaneuraOu 定跡フォーマット仕様（DB2016 / YBB）](yaneuraou.md)
- [NativeBook バイナリフォーマット仕様](book.md)
