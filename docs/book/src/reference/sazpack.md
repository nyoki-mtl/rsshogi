# sazpack（SAZ2）

## 用途

**sazpack** は、AlphaZero 系の自己対局から得た教師データを局単位で保存するバイナリ形式です。
magic は `SAZ2`、version は `2` です。
decoder は version `2` の chunk を読み込み、別の version には `UnsupportedVersion` を返します。

SAZ2 は開始局面の `PackedSfen` と各局面で指した手を保存します。
利用側で対局を再生して履歴特徴量を復元し、必要に応じて各 `Move` を学習用のラベルへ変換します。

## 保存する情報

1 局のレコードは次の情報を持ちます。

- 開始局面の `PackedSfen`
- `GameResult`
- 終局理由
- 入玉宣言ルール
- 各手を指す直前の局面レコード

各局面レコードは次の情報を持ちます。

- 実際に指した手
- 探索時の root WDL（勝ち、引き分け、負けの分布）
- 完局後に確定した outcome WDL
- 終局までの残り手数
- 要求した探索の訪問回数
- 教師データの重み
- 探索設定を表すフラグ
- 詰み手数の教師値（探索で証明した場合）
- 手ごとの事前確率、探索前後の訪問回数、勝敗の上下限
- ネットワークが出力した raw policy、raw WDL、raw mate、raw moves-left

root WDL と outcome WDL は別の教師です。
前者はその局面での探索評価を表し、後者は実際の終局結果を当該局面の手番視点へ変換した値です。

raw フィールドはこのどちらとも別で、探索を通す前のネットワーク出力です。
`raw_prior` は Dirichlet ノイズの付加や勝敗が証明された辺の抑制を行う前の事前確率、`raw_wdl` は探索で集約する前の WDL です。
探索定数の校正と、教師値の変換方法の評価を分けるために保存します。

`raw_moves_left` は moves-left head の予測手数で、対局結果から導く残り手数とは別の量です。
`raw_mate` は mate head の確率、`mate` は探索が証明した詰み手数です。

## Rust API

```rust,ignore
use rsshogi::records::formats::sazpack::{
    SazGame, SazOutcomeBound, SazPolicyEntry, SazPosition,
    SazTerminationReason, SazWdl, deserialize_chunk, serialize_chunk,
};

let bytes = serialize_chunk(&games)?;
let decoded: Vec<SazGame> = deserialize_chunk(&bytes)?;
```

`deserialize_chunk` は次の入力をエラーにします。

- magic または version の不一致
- ヘッダーの 6 バイト目の flags が `0` 以外（`UnsupportedFlags`）
- データの切り詰めと末尾の余分なバイト
- 未知の対局結果、終局理由、入玉宣言ルール、勝敗の上下限値
- mate の有無を表すタグが `0` / `1` 以外（`InvalidMateTag`）
- `visits_after < visits_before`
- prior、raw prior、または WDL（root、outcome、raw）の量子化値の総和が `65535` でないレコード
- `u32` として復号できない件数

`serialize_chunk` も分布の総和と訪問回数の条件を検査し、件数や payload 長が `u32` に収まらなければエラーにします。
これらの API は対局を再生しないため、開始局面の復元と各指し手の合法性検証は利用側で行います。

`deserialize_chunk_indexed` は、各対局と、その対局データが入力 chunk 内で占めるバイト範囲を返します。
chunk を分割して扱う場合に、元データの位置を記録できます。

## Python API

Python バインディングにも SAZ2 のレコード型を公開しています。

```python
from rsshogi import sazpack

data = sazpack.write_sazpack(games)
decoded = sazpack.decode_sazpack(data)
```

この API は SAZ2 の内容確認と、読み込み後の書き戻しに利用できます。
大量のデータの復号、履歴復元、シャッフル、バッチ化には Rust core を直接組み込めます。

## Policy 教師の組み立て

各 policy entry は `prior`、`raw_prior`、`visits_before`、`visits_after` を持ちます。
探索木を再利用する場合、`visits_after` は今回の探索以前に蓄積された訪問回数を含みます。
今回の探索で増えた回数は `visits_after - visits_before` で得られます。

学習処理では、累積回数と差分回数のどちらを policy 教師にするかを選びます。
選択した方式は、実験設定とチェックポイントの生成記録に残します。

## 値の量子化

`SazWdl` の `win`、`draw`、`loss` と、policy entry の `prior` および `raw_prior` は `u16` で保存します。
各分布は総和 `65535` で量子化します。
`prior` と `raw_prior` はそれぞれ独立した分布です。

`raw_mate` は mate head の確率を `u16 / 65535` で保存します。
`raw_moves_left` は予測手数を `手数 * 32` の固定小数で保存し、分解能は 1/32 手です。

`target_weight_milli` は教師データの重みを千分率で保存します。
例えば `1000` は重み 1.0、`750` は重み 0.75 を表します。

## 関連

- [sbinpack v2 仕様](./sbinpack.md)
- [Policy ラベル（内部）](../internals/types/policy-labels.md)
