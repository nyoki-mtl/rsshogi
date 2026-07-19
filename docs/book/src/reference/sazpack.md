# sazpack（SAZ2）

## 用途

**sazpack**は、AlphaZero系の自己対局から得た教師データを局単位で保存するバイナリ形式です。
現行schemaのmagicは`SAZ2`、versionは`1`です。
旧`SAZ1`との後方互換性はありません。

SAZ2は局面そのもののfeature tensorを保存しません。
開始局面の`PackedSfen`と各局面で指した手を保存し、loaderが対局をreplayして履歴featureを復元します。
policy labelも保存せず、各`Move`を読み出し時に現行のlabel体系へ変換します。

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
- optionalなmate教師
- 合法手ごとのprior、探索前後のvisit数、game-theoretic bounds

root WDLとoutcome WDLは別の教師です。
前者はその局面での探索評価を表し、後者は実際の終局結果を当該局面の手番視点へ変換した値です。

## Rust API

```rust,ignore
use rsshogi::records::formats::sazpack::{
    SazGame, SazOutcomeBound, SazPolicyEntry, SazPosition,
    SazTerminationReason, SazWdl, deserialize_chunk, serialize_chunk,
};

let bytes = serialize_chunk(&games)?;
let decoded: Vec<SazGame> = deserialize_chunk(&bytes)?;
```

`serialize_chunk`と`deserialize_chunk`は次の不正入力を拒否します。

- magicまたはversionの不一致
- payloadの切り詰めと末尾余剰byte
- 未知の終局理由、入玉宣言ルール、bounds
- `visits_after < visits_before`
- priorまたはWDLの量子化値の総和が`65535`でないrecord
- `u32`で表現できない件数

## Python API

Python bindingにもSAZ2のtyped recordを公開しています。

```python
from rsshogi import sazpack

data = sazpack.write_sazpack(games)
decoded = sazpack.decode_sazpack(data)
```

このAPIはSAZ2のinspectionとround-tripに利用できます。
学習pipelineがPyPI公開版`rsshogi`を必須依存にすることは想定していません。
大規模なdecode、履歴復元、shuffle、batch化は、利用側projectがRust coreを直接使う構成を推奨します。

## Policy教師の組み立て

各policy entryは`prior`、`visits_before`、`visits_after`を持ちます。
tree reuseが有効な場合、`visits_after`は今回の探索以前に蓄積されたvisitを含みます。
今回の探索だけで増えたvisit数は`visits_after - visits_before`で得られます。

どちらをpolicy教師に使うかはSAZ2 codecでは決めません。
学習pipelineが実験設定として明示し、manifestとcheckpoint provenanceへ記録します。

## 値の量子化

`SazWdl`の`win`、`draw`、`loss`と、policy entryの`prior`は`u16`で保存します。
各分布の総和は`65535`でなければなりません。

`target_weight_milli`は教師weightを千分率で保存します。
例えば`1000`はweight 1.0、`750`はweight 0.75を表します。

## 関連

- [sbinpack v2仕様](./sbinpack.md)
- [Policyラベル（内部）](../internals/types/policy-labels.md)
