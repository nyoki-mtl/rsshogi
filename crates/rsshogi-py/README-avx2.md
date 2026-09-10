# rsshogi-avx2

`rsshogi-avx2` は、AVX2 対応 x86_64 CPU 向けに最適化した `rsshogi` パッケージです。
import 名は標準版と同じ `rsshogi` で、公開 API は共通です。

> English: `rsshogi-avx2` is an AVX2-optimized build of the `rsshogi` Python
> module for AVX2-capable x86_64 CPUs. Use `rsshogi` for the standard build.

標準ビルドが必要な場合は [`rsshogi`](https://pypi.org/project/rsshogi/) を利用してください。

## インストール

```bash
python -m pip install rsshogi-avx2
```

## 注意

- `rsshogi` と `rsshogi-avx2` は同じ import 名を提供するため、環境ごとにどちらか一方を利用してください。
- このビルドは AVX2 対応の x86_64 CPU が必要です。迷う場合は `rsshogi` を利用してください。
- 標準版と AVX2 版の公開 API、合法手集合、wire format は共通です。

## ドキュメント

API リファレンスは mdBook ドキュメントにまとめています。

- [ドキュメント全体](https://nyoki-mtl.github.io/rsshogi/)
- [Python API（Board / Record など）](https://nyoki-mtl.github.io/rsshogi/python/index.html)
