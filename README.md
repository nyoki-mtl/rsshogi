# rsshogi

[crates.io](https://crates.io/crates/rsshogi) |
[docs.rs](https://docs.rs/rsshogi) |
[Documentation](https://nyoki-mtl.github.io/rsshogi/) |
[PyPI](https://pypi.org/project/rsshogi/) |
[Python 3.10+](https://nyoki-mtl.github.io/rsshogi/getting-started/installation.html) |
[MIT License](LICENSE)

Rust で実装された将棋ライブラリです。Python バインディングも提供しており、盤面管理、指し手生成、棋譜処理などの基本機能を高速に利用できます。

## 主な機能

- **盤面操作**: SFEN/USI 形式での読み書き、指し手の適用と取り消し
- **合法手生成**: 高速な合法手と擬似合法手の列挙
- **状態判定**: 王手、詰み、千日手、入玉宣言勝ちの判定
- **棋譜処理**: `Record` API による KIF/KI2/CSA/JKF/USI position の読み書き、メタデータ、終局情報、コメントの保持
- **定跡**: 大規模定跡の高速参照、DB2016 / YBB / SBK の外部定跡参照、DB2016 / SBK の書き出し
- **学習データ**: sbinpack v2、sazpack、PackedSfen / HCPE / pack 系フォーマット

## インストール

### Python

```bash
pip install rsshogi
```

AVX2 版（AVX2 対応 x86_64 CPU 専用、より高速）:

```bash
pip install rsshogi-avx2
```

※ 迷った場合は通常版の `rsshogi` を使ってください。`rsshogi` と `rsshogi-avx2` は同時にインストールできません。

### Python パッケージ名と import 名

- `rsshogi`: 標準配布（推奨）
- `rsshogi-avx2`: AVX2 最適化版（AVX2 対応 x86_64 CPU 専用）

どちらも **import 名は `rsshogi`** です。

### Rust

```toml
[dependencies]
rsshogi = "1.0.2"
```

## クイックスタート

### Python

```python
from rsshogi.core import Board
from rsshogi.record import Record

# 盤面操作
board = Board()
board.apply_usi("7g7f")
board.apply_usi("3c3d")
print(board.to_sfen())

# 合法手の列挙
for move in board.legal_moves():
    print(move.to_usi())

# 棋譜の読み込み
record = Record.from_kif_file("example.kif")
print(record.metadata.black_player)
```

### Rust

```rust
use rsshogi::board;

fn main() {
    let pos = board::hirate_position();
    println!("{}", pos.to_sfen(None));
}
```

Rust API の破壊的変更（`rsshogi::core` 廃止、`Position::to_move` 改名など）は
[`CHANGELOG.md`](CHANGELOG.md) を参照してください。

## ドキュメント

- **[ドキュメント（mdBook / GitHub Pages）](https://nyoki-mtl.github.io/rsshogi/)** - ガイド、API、内部仕様を含む総合ドキュメント
- **[Rust API リファレンス（docs.rs）](https://docs.rs/rsshogi)** - Rust API の詳細

## サンプルコード

- [Python サンプル](examples/python/)
- [Rust サンプル](examples/rust/)

## ライセンス

MIT License
