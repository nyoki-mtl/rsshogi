# インストール

## Python

### PyPI からインストール

```bash
pip install rsshogi
```

Python 3.10 以降をサポートしています。

### AVX2 版（AVX2 対応 x86_64 CPU 専用）

AVX2 対応 x86_64 CPU では AVX2 最適化版を利用できます。
幅広い CPU で動かす環境には通常版が適しています。

```bash
pip install rsshogi-avx2
```

> `rsshogi` と `rsshogi-avx2` は同じ import 名を使うため、環境ごとにどちらか一方をインストールします。
> 迷った場合は通常版の `rsshogi` を使ってください。

### インストールの確認

```python
import rsshogi
print(rsshogi.__version__)
```

## Rust

`Cargo.toml` に以下を追加してください。

```toml
[dependencies]
rsshogi = "1.2.2"
```

### Git リポジトリから

最新の開発版を使用する場合:

```toml
[dependencies]
rsshogi = { git = "https://github.com/nyoki-mtl/rsshogi" }
```

## 次のステップ

- [クイックスタート](quickstart.md)：基本的な使い方
- [例とパターン](examples.md)：実践的なコード例
