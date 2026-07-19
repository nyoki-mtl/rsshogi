# 特殊ルール

> **前提知識**: [ムーブジェネレータ](./index.md)（指し手生成の基本フロー）

## このページの要点

- 打ち歩詰めは「歩を打って王手 → 玉が逃げられない → 取り返せない」の 3 条件を**すべて**満たす場合のみ禁じ手
- 二歩判定は `筋マスク & 自歩 Bitboard` の交差で判定できる（最も安価な特殊ルールの一つ）
- 千日手検出は `apply_move32()` のたびに `repetition_counter` を差分更新し、`is_repetition(threshold)` は O(1) 判定
- 256 手ルールは大会ごとに最大手数が異なる（WCSC: 320, 電竜戦: 512, floodgate: 256）

将棋における特殊ルール（千日手、打ち歩詰め、二歩、ピン判定、入玉宣言）の実装とその注意点について解説します。

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

連続王手の千日手は `repetition_state()` の戻り値で判定します。
`is_perpetual_check()` という API は存在しません。
`continuous_check` カウンタが `apply_move32()` のたびに更新され、
連続王手側は `RepetitionState::Lose` / 受けている側は `RepetitionState::Win` が返ります。

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

## 256手ルール（最大手数ルール）

### ルール概要

対局が256手に達した場合、257手目の局面は送信されず、引き分けとなります。
ただし、256手目で詰みが発生している場合は、その時点で勝敗が決します。

### 実装上の落とし穴

256手ルールの実装には、詰み判定との相互作用に関する微妙な問題があります。

#### よくある実装ミス

多くのエンジンは、以下のような単純な実装をしてしまいます：

```rust,ignore
// ❌ 間違った実装例（is_max_moves_draw() は rsshogi に存在しない）
if position.game_ply() >= 256 {
    return Score::DRAW;  // 詰みチェックをせずに引き分け判定
}
```

rsshogi では手数は `pos.game_ply()` で取得できます。詰み判定は探索エンジン側で実装します。

この実装の問題点は、256手目で実際には詰んでいるにもかかわらず、引き分けと判定してしまう可能性があることです。

#### 正しい判定順序

理想的には、以下の順序で判定すべきです：

1. **256手に達したかをチェック**
2. **詰みかどうかをチェック**
3. 詰みでなければ引き分け

しかし、詰み判定は計算コストが高いため、多くのエンジンは探索の中で段階的に詰みを検出します。
これにより、256手ルールと詰み判定の順序が曖昧になる可能性があります。

#### 実用的な対処法

やねうら王の開発者が推奨する対処法は、**最大手数を少し高めに設定する**ことです：

```rust,ignore
const MAX_MOVES_TO_DRAW: usize = 258;  // 256より少し高めに設定

if position.game_ply() >= MAX_MOVES_TO_DRAW {
    return Score::DRAW;
}
```

この方法により、256手ルール付近での詰み判定の曖昧さを回避できます。

#### 大会ごとの最大手数の違い

実際の大会では、最大手数が異なることがあります：

- **WCSC（世界コンピュータ将棋選手権）**: 320手
- **電竜戦**: 512手
- **floodgate**: 256手

エンジンは、USI プロトコルを通じて最大手数を設定できるようにすることが推奨されます：

```text
# USI setoption での設定例
setoption name MaxMovesToDraw value 320
```

### パフォーマンスと正確性のトレードオフ

詰み判定を毎手実行するのは現実的ではありません。
そのため、以下のような段階的アプローチが一般的です：

1. **探索中の詰み検出**: アルファベータ探索で自然に検出
2. **最大手数付近での特別処理**: 250手を超えたあたりから詰みチェックを強化
3. **猶予付き最大手数**: 256ではなく258などに設定

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

rsshogi では `can_declare_impasse()` / `calculate_impasse_score()` という API は存在しません。
入玉宣言の判定は `declaration_win_move()` メソッドで行い、`EnteringKingRule` に応じて
ポイント制・トライルールを切り替えます。

```rust,ignore
use rsshogi::types::{EnteringKingRule, MOVE_NONE};

// 入玉宣言勝ちの手を取得（宣言できなければ MOVE_NONE を返す）
let win_move = pos.declaration_win_move();
if win_move != MOVE_NONE {
    // 宣言勝ちできる
}
```

実際のソースコードでは、`declaration_win_move` メソッドで入玉宣言勝ちを判定します。
`EnteringKingRule` に応じてポイント制・トライルールを切り替えます：

```rust,ignore
{{#include ../../../../../crates/rsshogi/src/board/position/rules.rs:position_declare_win}}
```
<small>[ソースコード](https://github.com/nyoki-mtl/rsshogi/blob/main/crates/rsshogi/src/board/position/rules.rs#L552-L675)</small>

## 実装チェックリスト

引き分け判定を実装する際は、以下の点をチェックしてください：

- [ ] 千日手検出でハッシュ値の衝突を考慮しているか
- [ ] 連続王手の千日手を正しく判定しているか
- [ ] 256手ルールで詰み判定との順序を考慮しているか
- [ ] 最大手数を設定可能にしているか（WCSC, 電竜戦など）
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
    // ... 過去局面と比較するには state_stack を遡る必要があり、
    // rsshogi の公開 API からは直接アクセスできない。
    // 通常は repetition_counter の精度で十分。
    true
}
```

### 256手ルール付近のテスト

256手付近の局面をテストケースとして用意し、詰みと引き分けの判定が正しいことを確認します：

```rust,ignore
#[test]
fn test_256_move_rule_with_mate() {
    use rsshogi::movegen::{Legal, generate_moves};
    use rsshogi::board::MoveList;

    let mut pos = position_from_sfen("position at 255 moves").unwrap();

    // 256手目で詰みの手を指す
    let mate_move = /* 合法手リストから詰み手を取得 */;
    pos.apply_move32(mate_move);

    // 合法手が0かつ王手中なら詰み
    let mut moves = MoveList::new();
    generate_moves::<Legal>(&pos, &mut moves);
    let is_checkmate = moves.is_empty() && pos.is_in_check();
    assert!(is_checkmate);

    // 手数確認は game_ply() で
    assert_eq!(pos.game_ply(), 256);
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
- **256 手ルール**: 詰みとの順序に注意、大会ごとに最大手数が異なる
- **入玉宣言**: 点数計算（大駒 5 点、小駒 1 点）+ 駒数チェック

## 次に読む

→ **[SFEN パーサ](../serialization/index.md)**: 局面のシリアライズ・デシリアライズに進みます。

## 参考資料

- [256手ルールの実装を間違えていた話 - やねうら王公式サイト](https://yaneuraou.yaneu.com/2021/01/13/incorrectly-implemented-the-256-moves-rule/) - 256手ルール実装の落とし穴
- [floodgate ルール](http://wdoor.c.u-tokyo.ac.jp/shogi/floodgate.html) - 最大手数256手の規定
- [コンピュータ将棋協会 - 大会ルール](https://www.computer-shogi.org/) - WCSC等の最大手数
