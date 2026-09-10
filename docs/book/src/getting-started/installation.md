# インストール

## Python

### PyPI からインストール

```bash
python -m pip install rsshogi
```

Python 3.10 以降をサポートしています。

### AVX2 版（AVX2 対応 x86_64 CPU 専用）

AVX2 対応 x86_64 CPU では AVX2 最適化版を利用できます。
幅広い CPU で動かす環境には通常版が適しています。

```bash
python -m pip install rsshogi-avx2
```

> `rsshogi` と `rsshogi-avx2` は同じ import 名を使うため、環境ごとにどちらか一方をインストールします。
> 迷った場合は通常版の `rsshogi` を使ってください。

同じ環境で版を切り替える場合は、両方の配布パッケージをアンインストールしてから、使う方を入れ直します。

```console
python -m pip uninstall -y rsshogi rsshogi-avx2
python -m pip install rsshogi-avx2
```

### インストールの確認

```python
import rsshogi
print(rsshogi.__version__)
```

## Rust

`Cargo.toml` に以下を追加してください。
Rust 1.95 以降が必要です。

```toml
[dependencies]
rsshogi = "1.2.4"
```

### Git リポジトリから

公開リポジトリの既定ブランチを使用する場合は、Git 依存を指定します。
再現可能なビルドには、使用するコミットを `rev` で固定してください。

```toml
[dependencies]
rsshogi = { git = "https://github.com/nyoki-mtl/rsshogi" }
```

## 次のステップ

- [クイックスタート](quickstart.md)：基本的な使い方
- [例とパターン](examples.md)：実践的なコード例
