# rsshogi マニュアル

`rsshogi` は、将棋局面、合法手生成、棋譜変換、定跡、学習データ形式を扱う MIT ライセンスの Rust ライブラリと Python パッケージです。

本マニュアルはバージョン 1.2.0 の公開 API と、呼び出し側から観測できる形式・意味論を記述します。
Rust の完全な item-level API は docs.rs、Python の型と引数は同梱の型スタブを正とし、本書では安全に組み込むための契約と代表的な使い方を説明します。

## 入口

- Rust から使う場合は [Rust API](rust-api.md) を参照します。
- Python から使う場合は [Python API](python-api.md) を参照します。
- 合法手、成り、駒打ち、一手詰めの意味論は [盤面の意味論](semantics.md) を参照します。
- 永続データを読み書きする場合は [形式と互換性](formats.md) を参照します。
- 1.1.1 から更新する場合は [1.2.0 への移行](migration.md) を先に確認します。

## 互換性の境界

1.2.0 は次の項目を互換契約として扱います。

- 文書化された public type とその有効な raw 値。
- SFEN、USI、KIF、KI2、CSA、JKF の公開された変換結果。
- HCP、PackedSfen、PACK、HCPE、YBB、SBK、SAZ2 の文書化された wire semantics。
- 合法手の集合、王手回避、成り、駒打ち、打ち歩詰め、局面更新・巻き戻し、一手詰めの意味論。

HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key は 1.1.1 と同じ表現を維持するため、1.2.0 への更新だけを理由に既存データを再生成する必要はありません。

一方、生成手の順序、内部 table の配置、非公開 helper、SIMD lane、命令選択は契約ではありません。
順序が必要な探索や UI は、呼び出し側で sort または score を行います。

## エラー処理

外部データの decoder は、切り詰め、無効な code、過剰な駒在庫、非正規な手、矛盾する勝敗を error として返します。
入力の検証に panic を利用しません。
Rust では失敗し得る constructor と `Result` を、Python では対応する例外を処理してください。

## バージョンの確認

Rust と Python の package version はどちらも 1.2.0 です。
Python では次のように実行時の版数を確認できます。

```python
import rsshogi

assert rsshogi.__version__ == "1.2.0"
```
