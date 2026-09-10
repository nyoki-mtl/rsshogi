# rsshogi

`rsshogi` は、将棋局面の操作、合法手生成、一手詰め判定、棋譜や定跡の入出力に使う Rust ライブラリです。

```rust
use rsshogi::board;

let position = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
println!("{}", position.to_sfen(None));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API と機能

常時利用できるモジュールは `board`、`mate`、`types` と、クレートルートに再エクスポートされた `movegen` です。
棋譜、定跡、policy ラベル、局面の直列化、SVG、検証、駒落ちなどの初期局面は、対応する Cargo feature で有効化します。

`Position` は局面状態を表します。
SFEN の解析は失敗し得るため、戻り値の `Result` を処理してください。
`to_sfen` は現在の局面を SFEN へ整形します。

不成を含むすべての合法手を生成するには `LegalAll` を使います。
`Legal` は探索向けに一部の不成を省きます。
王手回避や駒取りなど、用途別の生成モードもあり、各モードの合法性検査の範囲は異なります。
生成順は API の保証に含まれません。
`MoveList` と `Move32List` は、手生成に使う固定容量のコンテナで、ヒープ領域を確保しません。

`Move` は 16 ビットの指し手、`Move32` は移動後の駒情報を加えた 32 ビットの指し手です。
`Ki2Notation` は 1 手分の KI2 記法をヒープ確保なしで生成し、比較、ハッシュ、文字列変換、局面上の合法手への解決に使えます。

基本型の文書化された raw 値は、外部データとの互換性を保つための契約です。
外部入力には入力検証を行うコンストラクタを使い、検証なしで作った値をそのまま盤面アクセスに渡さないでください。
`Square` の添字は `file_index * 9 + rank_index` で、筋と段の index はともに 0 起点です。
USI 表記では筋を `1` から `9`、段を `a` から `i` で表します。

`solve_mate_in_one` は、合法で王手を与え、相手に合法応手がない場合だけ `Move32` を返し、それ以外では `None` を返します。
複数の詰みがある場合、任意の有効な手が返され得ます。
変更可能な探索局面を保持している場合は、`solve_mate_in_one_in_place` で局面の複製を避けられます。
この API は正常終了時に局面を復元します。
呼び出し前には、状態スタックを現局面と同期させておく必要があります。

HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key は文書化されたワイヤ表現を使用します。
PACK の指し手は `AperyMove` のビット配置を使うため、通常の `Move` の raw 値と受け渡す際は明示的に変換してください。

各 API とデータ形式の詳細は [rsshogi マニュアル](https://nyoki-mtl.github.io/rsshogi/) を参照してください。

## ライセンス

MIT。
