# インストール

この文書は rsshogi 1.5.0 を対象とする。
Native Book は独立圧縮ブロックを持つ wire version 4 を使用する。
旧定跡 API の削除を含むため、更新時は移行手順を確認する。

## Python パッケージの導入

Python 3.10 以降に対応する。
通常の環境では標準パッケージ `rsshogi` をインストールする。

```console
python -m pip install rsshogi
```

### AVX2 最適化版（x86_64 CPU 向け）

AVX2 および BMI1 命令に対応する x86_64 CPU 環境では、SIMD 命令を有効化してビルドされた `rsshogi-avx2` を選択できる。

```console
python -m pip install rsshogi-avx2
```

> [!IMPORTANT]
> `rsshogi` と `rsshogi-avx2` はどちらも同じ `rsshogi` モジュール名で展開されるため、同一の仮想環境に両方を同時インストールしてはならない。
> パッケージを切り替える際は、必ず既存のパッケージをアンインストールしてから入れ直す。

```console
python -m pip uninstall -y rsshogi rsshogi-avx2
python -m pip install rsshogi-avx2
```

### インストールの確認

```python
import rsshogi

print(rsshogi.__version__)
```

---

## Rust クレートの導入

Rust 1.95 以降（Edition 2024）が必要である。
`book` feature は Zstandard の C 実装をビルドするため、対象環境に対応する C コンパイラも必要である。
Python パッケージを sdist からビルドする場合も同様であり、配布済み wheel のインストールには不要である。
`Cargo.toml` の `[dependencies]` に `rsshogi` を追加する。

```toml
[dependencies]
rsshogi = "1.5.0"
```

### 用途別の Feature フラグ指定

`rsshogi` はデフォルト（`default = []`）では盤面操作、合法手生成、1 手詰め判定のみを有効にし、外部依存を最小限に抑えている。
用途に応じて必要な Feature フラグを有効化する。

| 用途 | `Cargo.toml` の設定例 |
|---|---|
| **USI 将棋エンジン（64-bit ハッシュ）** | `rsshogi = "1.5.0"` |
| **USI 将棋エンジン（128-bit ハッシュ + 定跡参照）** | `rsshogi = { version = "1.5.0", features = ["hash-128", "book"] }` |
| **棋譜変換、定跡構築、教師データ生成ツール** | `rsshogi = { version = "1.5.0", features = ["records", "book"] }` |
| **AlphaZero 系探索（Policy ラベルのみ利用）** | `rsshogi = { version = "1.5.0", features = ["policy-labels"] }` |
| **盤面エディタ、GUI バックエンド（局面検証 + SVG）** | `rsshogi = { version = "1.5.0", features = ["validation", "svg", "records"] }` |
| **全機能の有効化** | `rsshogi = { version = "1.5.0", features = ["python-data", "hash-128"] }` |

各 Feature が有効化するモジュールと依存クレートの詳細は [全体アーキテクチャの Cargo Feature フラグ一覧](../architecture.md#cargo-feature-フラグ一覧) を参照。

### Git リポジトリからの導入

公開リポジトリの最新コミットを利用する場合は `git` を指定する。
ビルドの再現性を保つため、`rev` または `tag` の固定を推奨する。

```toml
[dependencies]
rsshogi = { git = "https://github.com/nyoki-mtl/rsshogi", tag = "v1.5.0", features = ["records"] }
```

## 次のステップ

- [クイックスタート](quickstart.md)：局面の作成、指し手の適用、合法手生成、棋譜の読み書き。
- [実践レシピ集](examples.md)：対局シミュレーション、探索、棋譜の一括処理、局面分析のコード例。
