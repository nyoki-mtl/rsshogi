# 特殊ルール

> **前提知識**: [ムーブジェネレータ](./index.md)（指し手生成の基本フロー）

## このページの要点

- 打ち歩詰めは「歩を打って王手 → 玉が逃げられない → 取り返せない」の 3 条件を**すべて**満たす場合のみ禁じ手
- 二歩判定は `筋マスク & 自歩 Bitboard` の交差で判定できる（最も安価な特殊ルールの一つ）
- 千日手検出は `apply_move32()` のたびに `repetition_counter` を差分更新し、`is_repetition(threshold)` は O(1) 判定
- 最大手数は大会ごとに違うため設定可能にする。引き分け判定は詰み判定より後に置く

将棋のルールの大半は「その駒がどこへ動けるか」で書けます。
残りが厄介です。
二歩は盤面だけで決まりますが、千日手は履歴を、打ち歩詰めは打った先の局面を見ないと判定できません。
ピンに至っては、動かしてはじめて違法とわかる手です。
このページでは、こうした「駒の動きの表に載らないルール」を 1 つずつ実装に落とします。

## 王手判定

王の位置から敵駒の攻撃範囲を逆算し、実際の敵駒配置と照合します。

rsshogi では王手判定は `Position::is_in_check()` で行います（引数なし、手番側の玉への王手を返す）。
王手をかけている駒の集合は `pos.checkers()` で取得できます（`apply_move32()` のたびに自動キャッシュ）。

以下は概念的な王手判定のコード（実際には `is_in_check()` が内部で同様の処理を行います）：

```rust,ignore
// rsshogi の公開 API での使い方
let in_check: bool = pos.is_in_check();
let checkers: Bitboard = pos.checkers();  // 王手している駒の集合

// 特定マスへの攻撃駒を列挙したい場合（attacks.rs）
let occupied = pos.bitboards().occupied();
let attackers: Bitboard = pos.attackers_to(sq, occupied);
```

Bitboard 実装では、駒種別の攻撃候補を集合として合成し、現在の敵駒配置と照合します。
配列ベースで全敵駒を走査する実装に比べ、分岐とループを抑えやすい構造です。

## ピン判定（Pin Detection）

<div id="live-pin-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-pin-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  // 5五に先手玉、5三に先手銀（ピンされている）、5二に後手飛の直線。
  board.setPositionFromSFEN('9/4r4/4S4/9/4K4/9/9/9/9 b - 1');
  board.goTo(0);
  // ピンの直線を矢印で示す
  board.setArrows([
    { from: '5b', to: '5e', color: 'rgba(220,50,50,0.6)', width: 1.2 },
  ]);
  // ピンされている銀に丸
  board.setCircles([
    { square: '5c', color: 'rgba(255,140,0,0.8)', width: 2.0 },
  ]);
  window.livePin1 = board;
}
</script>

王と敵の遠方駒の間に自駒が 1 つだけある場合、その自駒は「ピン」されています。
上の図では、5二の後手飛車と5五の先手玉の間にある5三の先手銀がピンされています。

```rust,ignore
pub fn compute_pinned_pieces(position: &Position, king_color: Color) -> Bitboard {
    let king_sq = position.king_square(king_color);
    let bitboards = position.bitboards();
    let self_pieces = bitboards.by_color[king_color];
    let enemy_color = king_color.flip();
    let occupied = bitboards.occupied;

    let mut pinned = Bitboard::EMPTY;

    // 飛車・竜のピン
    let rook_pinners = bitboards.by_piece[ROOK] & bitboards.by_color[enemy_color];
    for pinner_sq in rook_pinners {
        let between = Bitboard::between(king_sq, pinner_sq);
        let blockers = between & occupied;

        // 間に駒が1つだけ && それが自駒
        if blockers.count() == 1 && (blockers & self_pieces).any() {
            pinned |= blockers;
        }
    }

    // 角・馬のピン（同様の処理）
    // 香のピン（同筋の場合のみ）
    // ...

    pinned
}
```

**参照実装の対応実装**: `blockersForKing` / `pinners` キャッシュ

## 二歩判定

<div id="live-nifu-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-nifu-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: true });
  // 二歩のテスト局面（実テストより）: FILE_2とFILE_8以外に歩が存在するため、
  // 二歩ルールにより歩を打てるのはFILE_2とFILE_8のみ
  board.setPositionFromSFEN('lnsgk1snl/6gb1/p1pppp2p/6pR1/9/1rP6/P2PPPP1P/1BG6/LNS1KGSNL b 2P2p 1');
  board.goTo(0);
  // 黄色ハイライト: 二歩に該当せず、かつ空いている歩打ち候補
  board.highlightSquares([
    (2 - 1) * 9 + (3 - 1), // ２三
    (2 - 1) * 9 + (5 - 1), // ２五
    (2 - 1) * 9 + (6 - 1), // ２六
    (2 - 1) * 9 + (7 - 1), // ２七
    (2 - 1) * 9 + (8 - 1), // ２八
    (8 - 1) * 9 + (3 - 1), // ８三
    (8 - 1) * 9 + (4 - 1), // ８四
    (8 - 1) * 9 + (5 - 1), // ８五
    (8 - 1) * 9 + (7 - 1), // ８七
  ]);
  // 歩が打てない筋（既に歩がある）に赤丸
  board.setCircles([
    { square: '9g', color: 'rgba(220,50,50,0.4)' },
    { square: '7g', color: 'rgba(220,50,50,0.4)' },
    { square: '6g', color: 'rgba(220,50,50,0.4)' },
    { square: '5g', color: 'rgba(220,50,50,0.4)' },
    { square: '4g', color: 'rgba(220,50,50,0.4)' },
    { square: '3g', color: 'rgba(220,50,50,0.4)' },
    { square: '1g', color: 'rgba(220,50,50,0.4)' },
  ]);
  window.liveNifu1 = board;
}
</script>

同じ筋に自分の歩が既に存在するかを判定します。
上の局面では先手の歩が 2 筋と 8 筋を除くすべての筋に存在しているため、二歩ルールにより歩を打てる筋は 2 筋と 8 筋だけです。
黄色ハイライトは、その 2 筋・8 筋の中でも実際に空いていて、先手の歩が成れない一段目でもないマスです。
赤い丸は、既に先手歩があるため二歩で除外される筋を示しています。

```rust,ignore
pub fn can_drop_pawn(position: &Position, file: File, color: Color) -> bool {
    let bitboards = position.bitboards();
    let pawns = bitboards.by_piece[PAWN] & bitboards.by_color[color];
    let file_mask = Bitboard::file_mask(file);

    // 指定筋に歩がない
    !(pawns & file_mask).any()
}
```

実装上は、指定筋のマスクと自歩 Bitboard の交差が空かどうかを調べます。

## 打ち歩詰め判定

<div id="live-uchifuzume-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-uchifuzume-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: true });
  // 打ち歩詰めの実戦形（実テストより）:
  // 先手が P*1d と歩を打つと後手玉（1c）に王手がかかるが、
  // 逃げることも取ることもできないため打ち歩詰めとなり禁じ手。
  board.setPositionFromSFEN('l1+R2+R3/6ggl/p3ppppk/2p1b4/6S2/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b Pg2sn2l4p 1');
  board.goTo(0);
  // 打ち歩詰めとなる 1d への歩打ちを矢印で示す
  board.setArrows([
    { from: 'S:P', to: '1d', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  // 後手玉の位置を丸で強調
  board.setCircles([
    { square: '1c', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  window.liveUchifuzume1 = board;
}
</script>

歩を打った直後に王手がかかり、かつ王が逃げられない場合は禁じ手です。
上の局面（実テストコードより）では、先手が P\*1d（1四に歩を打つ）と後手玉（1三）に王手がかかりますが、後手玉は逃げることも歩を取ることもできないため打ち歩詰めとなり、この手は禁じ手です。

### 判定アルゴリズム

打ち歩詰め判定（`is_legal_drop`）は、歩を打つと王手になる前提で、以下の 3 条件を順に確認します。
いずれかで「逃れられる」と分かれば打ち歩詰めではなく、最後まで残れば打ち歩詰め（＝違法な歩打ち）です。

1. **取り返しチェック（ピンの同筋例外つき）**: 打った歩を相手が取り返せるか。
   取り返せる敵駒は `attackers_to_pawn(them, to)`（玉・香・歩を除く）で列挙し、
   そのうちピンされていない駒、または「歩と同じ筋」方向にピンされている駒（その方向には取れる）があれば取り返せる。
2. **玉の逃げ場チェック**: 歩を打った後の盤面で、玉が利きの及ばないマスへ逃げられるか。
3. 上記いずれにも当てはまらなければ打ち歩詰め。

> 「合駒」のステップは存在しません。歩を打つ手に対して合駒で受けることはできないためです。

実際のソースコードは `is_legal_drop` メソッドです：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/rules.rs:position_drop_pawn_mate}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/rules.rs#L250-L288)</small>

**ピン駒の同筋例外の図解**:

```text
例1) 斜めのピン        例2) 横のピン         例3) 縦のピン（例外）
^玉 ^角  飛            ^玉                   ^玉
 歩                     歩 ^飛                 歩
                                 角           ^飛
                                               香
```

- **例1, 2**: ピン駒が玉頭の歩を取る動きは、ピン方向と異なるため合法
- **例3**: ピン駒（飛）が玉頭の歩を取る動きは、ピン方向と一致するため合法（`file_bb` で例外処理）

この判定は単純な二歩判定より重く、取り返しチェック、ピン判定、玉の逃げ場判定を組み合わせます。
実装の要点は、歩を打った後の局面を完全に探索するのではなく、打ち歩詰めに必要な逃れ手だけを確認することです。

## キャッシュ戦略

王手・ピン・王手候補升などは `StateInfo` 内の `TacticalCache`（`pub(crate)`、外部からは
アクセサ経由で参照）にキャッシュされます。概念的には次の情報を保持します。

```rust,ignore
// 概念図（実体は state_info.rs の TacticalCache）
struct TacticalCache {
    checkers: Bitboard,                       // 王手している駒
    pinners: [Bitboard; 2],                   // ピンを掛けている駒（先後別）
    blockers_for_king: [Bitboard; 2],         // 玉へのラインを遮る駒（＝ピンされうる駒、先後別）
    check_squares: CheckSquares,              // 駒種別の王手候補升（CheckSquares = [Bitboard; 9] の newtype）
}
```

公開 API では `pos.checkers()` / `pos.blockers_for_king(color)` などのアクセサ経由で参照します。
これらをキャッシュすることで、合法手生成時の重複計算を回避できます。

## 千日手（Repetition）

### 基本ルール

同一局面が4回出現した場合、千日手として引き分けとなります。
ただし、連続王手の千日手は反則負けとなります。

千日手の状態は `RepetitionState` enum で表現されます：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/types/repetition_state.rs:repetition_state_enum}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/types/repetition_state.rs#L17-L35)</small>

### 実装パターン

千日手の検出には、StateInfo スタックに保存されたハッシュ値を使用します。
実際の実装では、`do_move` のたびにインクリメンタルに千日手カウンタを更新し、
`is_repetition` は単にカウンタを閾値と比較するだけです：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/rules.rs:position_is_repetition}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/rules.rs#L381-L502)</small>

### rsshogi の千日手判定実装

rsshogi では `apply_move32()` の中で `repetition_counter` を差分更新します（4手前から2手ずつ遡る O(1) 計算）。
`is_repetition(threshold)` はカウンタを閾値と比較するだけです。

将棋では同一手番の局面のみを比較するため、内部的には4手前から2手ずつ遡ります：

```text
局面A（先手番）
  ↓ 先手の指し手
局面B（後手番）
  ↓ 後手の指し手
局面C（先手番）← 局面Aと比較対象（2手ずつ遡る）
```

### 連続王手の千日手判定

連続王手の千日手は、専用の判定関数ではなく `repetition_state()` の戻り値で区別します。
`continuous_check` カウンタが `apply_move32()` のたびに更新され、
連続王手をかけた側には `RepetitionState::Lose`、受けていた側には `RepetitionState::Win` が返ります。

```rust,ignore
use rsshogi::types::RepetitionState;

// 千日手の種類を判定
match pos.repetition_state() {
    RepetitionState::None    => { /* 千日手ではない */ }
    RepetitionState::Draw    => { /* 引き分けの千日手 */ }
    RepetitionState::Win     => { /* 相手の連続王手千日手（手番側の勝ち）*/ }
    RepetitionState::Lose    => { /* 自分の連続王手千日手（手番側の負け）*/ }
    RepetitionState::Superior   => { /* 優等局面 */ }
    RepetitionState::Inferior   => { /* 劣等局面 */ }
}
```

## 最大手数ルール

### ルール概要

規定の手数に達しても勝敗が決まらなければ引き分けです。
手数は大会ごとに決められており、WCSC29 では 320 手、電竜戦では 512 手が使われました。[^yaneuraou-256]
ただし規定手数の局面で詰んでいれば、引き分けではなく負けです。

エンジン側は手数を固定値で埋め込まず、USI の option で受け取れるようにします。

```text
setoption name MaxMovesToDraw value 320
```

rsshogi は手数を `pos.game_ply()` で返すところまでを担当し、引き分け判定そのものは探索エンジン側の責務です。

### 実装上の落とし穴

素直に書くと、次のようになります。

```rust,ignore
if position.game_ply() >= max_moves_to_draw {
    return Score::DRAW;  // 詰みを見ずに引き分けを返している
}
```

これは規定手数の局面で詰まされていても引き分けを返します。
本来はその局面で詰みかどうかを先に見なければなりません。
順序が逆になるのは、詰み判定が重いからです。
探索は枝刈りを済ませて指し手を生成した後にはじめて「合法手なし＝詰み」を知るため、
安価な手数チェックのほうが自然と手前に来ます。

### 規定手数より数手多く設定する

やねうら王の開発者は、引き分けとみなす手数を規定より 2〜6 手多く設定することを勧めています。[^yaneuraou-256]
動機は判定順序の修正ではありません。
規定手数の付近でエンジンが引き分けと誤解したまま頓死するのを防ぐことです。

誤解は 2 つの経路で起こると説明されています。
1 つは詰み判定の取りこぼしです。
やねうら王の 1 手詰めルーチンはあらゆる 1 手詰めを解くわけではなく、たとえば離し飛車で合駒が利かない詰みを見落とします。
見落とせば、本来そこで終わっているはずの局面から先を読み進めてしまいます。
もう 1 つは置換表です。
規定手数が絡んで確定した引き分けスコアが登録されると、手数の浅い局面でそれを引いて引き分けと錯覚することがあります。

引き分けとみなす手数を規定より先に置いておけば、エンジン自身の引き分け判定が動く前に対局が終わります。
誤解が実際の勝敗に届かなくなる、というのがこの設定の効果です。

```rust,ignore
// 大会規定が 320 手なら、内部では 322〜326 手あたりを使う
let max_moves_to_draw = tournament_limit + 2;
```

## 持将棋（Impasse）

### ルール概要

両者の玉が敵陣に入り、膠着状態になった場合、点数計算により勝敗を決定します：

- **大駒（飛・角）**: 5点
- **小駒（その他）**: 1点

両者とも以下の条件を満たす場合、点数で判定：

- 先手：27点以上
- 後手：27点以上

両者とも27点以上なら引き分け。
どちらかが27点未満なら、その手番側の負け。

### 入玉宣言の実装

入玉宣言の可否は `declaration_win_move()` が返します。
点数計算と宣言可否を別々の関数に分けず、「宣言勝ちの手」を 1 つ返す形にまとめてあります。
宣言できない局面では `MOVE_NONE` が返ります。

```rust,ignore
use rsshogi::types::MOVE_NONE;

let win_move = pos.declaration_win_move();
if win_move != MOVE_NONE {
    // 宣言勝ちできる
}
```

ポイント制とトライルールの切り替えは `EnteringKingRule` に応じて内部で行われます。

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/rules.rs:position_declare_win}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/rules.rs#L552-L675)</small>

## 実装チェックリスト

引き分け判定を実装する際は、以下の点をチェックしてください：

- [ ] 千日手検出でハッシュ値の衝突を考慮しているか
- [ ] 連続王手の千日手を正しく判定しているか
- [ ] 最大手数による引き分けを、詰み判定より後に置いているか
- [ ] 最大手数を USI option で設定可能にしているか
- [ ] `declaration_win_move()` で入玉宣言勝ちを判定しているか
- [ ] `EnteringKingRule` の設定（ポイント制 / トライルール / なし）が適切か

## デバッグのヒント

### 千日手の誤検出を防ぐ

ハッシュ値の衝突により、異なる局面を同一と誤判定する可能性があります。
rsshogi の `is_repetition(threshold)` は `repetition_counter` のカウンタを使う O(1) 判定ですが、
ハッシュ衝突への対処が必要な場合はエンジン側で追加検証を行います：

```rust,ignore
// ハッシュ衝突を検出するための追加チェック（エンジン側の実装例）
fn is_repetition_strict(pos: &Position) -> bool {
    if !pos.is_repetition(3) {
        return false;
    }

    // SFEN 文字列で完全比較（重い処理のためデバッグ用途のみ）
    let current_sfen = pos.to_sfen(None);
    // 過去局面との比較には、エンジン側で指し手と SFEN の履歴を持つ。
    // 通常は repetition_counter の精度で十分。
    true
}
```

### 最大手数付近のテスト

規定手数の直前の局面を用意し、詰みと引き分けの判定順序が正しいことを確認します。

```rust,ignore
#[test]
fn test_mate_takes_precedence_over_max_moves() {
    use rsshogi::movegen::{Legal, generate_moves};
    use rsshogi::board::MoveList;

    let max_moves_to_draw = 320;
    let mut pos = position_from_sfen(sfen_at_ply(max_moves_to_draw - 1)).unwrap();

    // 規定手数ちょうどの局面で詰ます
    let mate_move = /* 合法手リストから詰み手を取得 */;
    pos.apply_move32(mate_move);

    // 合法手が0かつ王手中なら詰み
    let mut moves = MoveList::new();
    generate_moves::<Legal>(&pos, &mut moves);
    let is_checkmate = moves.is_empty() && pos.is_in_check();
    assert!(is_checkmate);

    // 規定手数に到達しているが、引き分けではなく詰みが優先される
    assert_eq!(pos.game_ply(), max_moves_to_draw);
}
```

## 落とし穴

### 打ち歩詰め vs 突き歩詰め

**打ち歩詰め**（持ち駒の歩を打って詰ます）は禁じ手ですが、**突き歩詰め**（盤上の歩を進めて詰ます）は合法です。
この区別は `mv.is_drop()` で判定しますが、見落としやすいバグの原因です。

### と金は二歩ではない

「と金」（成った歩）は駒種としては `PRO_PAWN` であり、`PAWN` ではありません。
筋の二歩チェックは `PieceType::PAWN` のみを対象にし、`PRO_PAWN` は含めません。
`PRO_PAWN` を二歩判定に含めてしまうと、と金がある筋に歩が打てなくなる重大なバグになります。

### 連続王手千日手の判定方向

連続王手千日手は「王手をかけ**続けた**側」の反則負けです。
現在王手されている局面から過去を遡り、すべての繰り返し局面で王手がかかっていたかを確認する必要があります。
「王手されている側が千日手」ではなく「王手している側が千日手」である点に注意してください。

## まとめ

- **王手判定**: 逆利き方式で候補集合を作り、敵駒配置と照合する
- **二歩判定**: 筋マスクと自歩 Bitboard の交差で判定する
- **打ち歩詰め**: 取り返しチェック、玉の逃げ場、ピン判定の同筋例外が要点
- **千日手**: `repetition_counter` の差分更新（O(1)）、連続王手は `RepetitionState::Win/Lose` で判定
- **最大手数**: 引き分け判定は詰み判定の後。規定より数手多めに設定して誤判定の影響を避ける
- **入玉宣言**: 点数計算（大駒 5 点、小駒 1 点）+ 駒数チェック

## 次に読む

→ **[SFEN パーサ](../serialization/index.md)**: 局面のシリアライズ・デシリアライズに進みます。

## 参考資料

- [コンピュータ将棋協会 - 大会ルール](https://www.computer-shogi.org/) - WCSC の規定

[^yaneuraou-256]: やねうら王, [「256手ルールの実装を間違えていた話」](https://yaneuraou.yaneu.com/2021/01/13/incorrectly-implemented-the-256-moves-rule/)（2021-01-13）。記事は規定手数で引き分けとなるルールを「256手ルール」と総称している。
