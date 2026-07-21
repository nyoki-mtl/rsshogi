# インストール

## Python

### PyPI からインストール

```bash
pip install rsshogi
```

Python 3.10 以降をサポートしています。

### AVX2 版（AVX2 対応 x86_64 CPU 専用）

AVX2 対応 x86_64 CPU では AVX2 最適化版を利用できます。通常版より高速ですが、AVX2 非対応 CPU では通常版を使ってください。

```bash
pip install rsshogi-avx2
```

!!! warning "注意"
    迷った場合は通常版の `rsshogi` を使ってください。
    `rsshogi` と `rsshogi-avx2` は同時にインストールできません。
    どちらか一方を選択してください。

### インストールの確認

```python
import rsshogi
print(rsshogi.__version__)
```

## Rust

`Cargo.toml` に以下を追加してください。

```toml
[dependencies]
rsshogi = "1.0.2"
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
