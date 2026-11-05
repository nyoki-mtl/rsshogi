# 1.7.4 の変更内容

1.7.4 は 2026-11-06 公開の、千日手判定の不具合を修正する後方互換な patch 更新である。
Rust と Python の公開 API のシグネチャ、および定跡と棋譜の保存形式は変更しない。

## 持駒差を挟む千日手の検出

盤上の駒と手番が同じでも、持駒が異なれば別の局面である。
完全一致する局面 A の過去の出現より近くに、持駒だけが異なる局面 B があると、優等または劣等の判定によって履歴走査が打ち切られ、A の反復回数を数え損ねる場合があった。
このため、A が 4 回現れていても、対局の終局判定や棋譜解析で成立済みの千日手を見落としていた。

1.7.4 では完全一致の出現回数と連続王手の分類を、最寄りの優等または劣等の判定から独立して保持する。
A、A、A、B、A の順に現れる履歴でも、最後の A で 4 回目の完全一致を検出する。
同じ盤面に戻る途中で捕獲や駒打ちによって持駒が増減する履歴が、この修正の対象となる。

## 対局と棋譜の判定

| API | 判定内容 |
|---|---|
| Rust `Position::is_repetition(3)`、Python `Board.is_repetition(3)` | 現局面と完全一致する過去局面が 3 回以上あり、現局面を含めて 4 回目に達したかを返す。 |
| Rust `Position::repetition_state()`、Python `Board.repetition_state()` | 成立した千日手を連続王手も含めて分類する。成立前の優等または劣等の判定も維持する。 |

棋譜のルートから指定ノードまでの履歴を使う `path_is_repetition(record, target, 3)` と `path_repetition_state(record, target)` にも同じ修正が適用される。
連続王手の `Win`、`Lose`、`Draw` の分類は、優等または劣等の分類と混同せずに継承する。
成立の有無だけが必要な場合は `is_repetition(3)` を使う。
`Superior` と `Inferior` は持駒の比較結果であり、規則上の千日手成立を意味しない。

Python の標準パッケージ `rsshogi` と AVX2 パッケージ `rsshogi-avx2` は、どちらも Rust core の修正を利用する。
既存の呼び出しを変更する必要はない。

## キャッシュと探索 API の互換性

完全一致の回数は持駒差の判定とは独立して数える。
そのため、`repetition_counter() > 0` でも `repetition_type()` が `Superior` または `Inferior` になり得る。
この場合の `repetition_distance()` は持駒が異なる候補までの距離であり、完全一致までの距離とは限らない。
完全一致が 4 回に達する前は、古い完全一致を見つけても、最寄りの優等または劣等の判定を上書きしない。

4 回目の完全一致では、成立した千日手を優先する。
`repetition_state_with_ply(0)` と `cached_repetition_state_with_ply(0)` も成立済みの千日手を返すため、探索開始局面での見落としも解消する。
一方、`repetition_state_with_distance(depth)` は指定範囲内の最寄りの判定を返す契約を維持する。
同じ局面でも、近くに持駒差の候補があれば、成立済みの千日手とは異なる種類を返し得る。
API ごとの距離境界は [探索エンジン連携プリミティブ](../internals/position/search-integration.md#3-千日手判定) を参照する。

null move より前や保持範囲外の履歴は走査しない。
履歴を短く切り詰めると、範囲外の一致に依存する千日手や連続王手を見逃し得る。
現在の SFEN だけでは過去の反復を復元できず、判定には着手によって保持した履歴が必要となる。

やねうら王の着手時キャッシュ計算には、優等または劣等で走査を終了する経路がある[^yaneuraou]。
今回の修正はその経路と挙動が異なるため、該当する履歴の判定や探索 node 数の一致は保証しない。

[^yaneuraou]: [YaneuraOu `position.cpp`、revision `bd6db90a7ac91875356f6e4ac61a18c9031a9116`](https://github.com/yaneurao/YaneuraOu/blob/bd6db90a7ac91875356f6e4ac61a18c9031a9116/source/position.cpp) の `ENABLE_QUICK_DRAW` が無効な着手時キャッシュ計算経路。
