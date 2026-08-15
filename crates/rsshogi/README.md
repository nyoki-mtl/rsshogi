# rsshogi

`rsshogi` は、将棋局面、合法手生成、一手詰め、棋譜・定跡 I/O に使う再利用可能な Rust の基本型を提供します。

```rust
use rsshogi::board;

let position = board::position_from_sfen(
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
)?;
println!("{}", position.to_sfen(None));
# Ok::<(), Box<dyn std::error::Error>>(())
```

## API と機能

常時利用できる API は `board`、`mate`、`types`、crate-root の `movegen` 再エクスポートです。
`records`、`book`、`labels`、局面の直列化、SVG、検証、初期局面対応は機能で有効化します。
選択した機能の API と形式契約は workspace manual を参照してください。

`Position` は完全な局面状態を表します。
SFEN の解析・整形は失敗し得ます。
合法手の生成は現在局面で有効な手だけを返し、生成順は意図的に未規定です。
完全な合法手集合には `LegalAll` を使います。`Legal` とその他の mode は探索向けに一部の手または合法性 filter を分割します。
`MoveList` と `Move32List` は生成手向けの固定容量・非割り当てコンテナです。
`Move` と `Move32` は、`Move32` のメタデータを除けば同じ正規化済みの手集合を記述します。

文書化されたプリミティブ型の公開 raw values は互換契約です。
外部/raw data には失敗し得る constructors を使用してください。
`Square` は file-major (`file_index * 9 + rank_index`) で、USI 座標は file 1–9、rank a–i です。

`solve_mate_in_one` は、合法で王手を与え、相手に合法応手がない場合だけ `Move32` を返し、それ以外では `None` を返します。
複数の詰みがある場合、任意の有効な手が返され得ます。

HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key は文書化されたワイヤ表現を使用します。
PACK の指し手は `AperyMove` layout なので、通常の `Move` raw 値との間は明示変換してください。

各 API とデータ形式の詳細は [rsshogi マニュアル](https://nyoki-mtl.github.io/rsshogi/) を参照してください。

## ライセンス

MIT。
