# sazpack（SAZ2）

## 用途

**sazpack**は、AlphaZero系の自己対局から得た教師データを局単位で保存するバイナリ形式です。
magic は `SAZ2`、version は `2` です。
decoder は version `2` の chunk を読み込み、別の version には `UnsupportedVersion` を返します。

SAZ2 は開始局面の `PackedSfen` と各局面で指した手を保存します。
loader は対局を replay して履歴 feature を復元し、各 `Move` を読み出し時の label 体系へ変換します。

## 保存する情報

一局のrecordは次の情報を持ちます。

- 開始局面の`PackedSfen`
- `GameResult`
- 終局理由
- 入玉宣言ルール
- 各手を指す直前の局面record

各局面recordは次の情報を持ちます。

- 実際に指した手
- 探索時のroot WDL
- 完局後に確定したoutcome WDL
- 終局までの残り手数
- 要求した探索visit数
- 教師weight
- exploration設定を表すflags
- optionalなmate教師（探索が証明した詰み手数）
- 合法手ごとのprior、探索前後のvisit数、game-theoretic bounds
- networkがそのまま出力したraw policy、raw WDL、raw mate、raw moves-left

root WDLとoutcome WDLは別の教師です。
前者はその局面での探索評価を表し、後者は実際の終局結果を当該局面の手番視点へ変換した値です。

raw fieldはこのどちらとも別で、探索を通す前のnetwork出力そのものです。
`raw_prior`はDirichlet noiseもproven-edge抑制もかける前のprior、`raw_wdl`は探索集約を経ないWDLです。
探索定数の校正とtarget変換の評価を分離するために保存します。

`raw_moves_left`はmoves-left headの予測手数で、対局結果から導く残り手数とは別の量です。
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

`serialize_chunk` と `deserialize_chunk` は次の条件を検証します。

- magicまたはversionの不一致
- header 6byte目のflagsが`0`以外（`UnsupportedFlags`）
- payloadの切り詰めと末尾余剰byte
- 未知の終局理由、入玉宣言ルール、bounds
- mate optionalタグが`0`/`1`以外（`InvalidMateTag`）
- `visits_after < visits_before`
- prior、raw prior、またはWDL（root、outcome、raw）の量子化値の総和が`65535`でないrecord
- `u32`で表現できない件数

## Python API

Python bindingにもSAZ2のtyped recordを公開しています。

```python
from rsshogi import sazpack

data = sazpack.write_sazpack(games)
decoded = sazpack.decode_sazpack(data)
```

この API は SAZ2 の inspection と round-trip に利用できます。
大規模な decode、履歴復元、shuffle、batch 化には Rust core を直接組み込めます。

## Policy教師の組み立て

各policy entryは`prior`、`raw_prior`、`visits_before`、`visits_after`を持ちます。
tree reuseが有効な場合、`visits_after`は今回の探索以前に蓄積されたvisitを含みます。
今回の探索だけで増えたvisit数は`visits_after - visits_before`で得られます。

学習 pipeline は `visits_after` または差分 visit のどちらを policy 教師にするかを選び、
実験設定を manifest と checkpoint provenance へ記録します。

## 値の量子化

`SazWdl`の`win`、`draw`、`loss`と、policy entryの`prior`および`raw_prior`は`u16`で保存します。
各分布は総和 `65535` で量子化します。
`prior` と `raw_prior` はそれぞれ独立した分布です。

`raw_mate`はmate headの確率を`u16 / 65535`で保存します。
`raw_moves_left`は予測手数を`手数 * 32`の固定小数で保存し、分解能は1/32手です。

`target_weight_milli`は教師weightを千分率で保存します。
例えば`1000`はweight 1.0、`750`はweight 0.75を表します。

## 関連

- [sbinpack v2仕様](./sbinpack.md)
- [Policyラベル（内部）](../internals/types/policy-labels.md)
