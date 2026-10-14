# 特殊ルールと王手および合法性判定

将棋の指し手生成と合法性検査には、チェスには存在しない持ち駒の制約（二歩、打ち歩詰め、行き所のない駒）や、王手回避、王手生成、入玉宣言勝ちといった特殊アルゴリズムが必要となる。
本ページでは、`evasions.rs`、`checks.rs`、`legality.rs`、および `rules.rs` におけるこれらの内部実装を解説する。

---

## 1. 王手回避手生成（`evasions.rs`）

手番側の玉に王手がかかっている局面（`pos.in_check() == true`）では、通常の指し手生成ではなく専用の `generate_evasions` が呼び出される。
`StateHot` の `checkers` ビットボードに立っている王手駒の数（`checkers.count()`）によって、処理は 2 つに分岐する。

### 両王手（`checkers.count() >= 2`）の場合

2 枚の敵駒から同時に王手を受けている場合、1 枚の敵駒を捕獲したり 1 本の利きを合い駒で遮断したりしても、もう片方の王手駒の利きが残る。
したがって、**玉自身が敵の利きのないマスへ逃げる手のみ** が唯一の回避手段となる。
生成処理は玉の移動先候補 8 マスの検査だけで即座に完了する。

### 単王手（`checkers.count() == 1`）の場合

王手駒が 1 枚（`checker_sq`）のみのときは、以下の 3 種類の回避手を生成する。

1. **玉自身の移動**：玉の周囲 8 マスのうち、味方駒がなく、かつ王手している飛び駒の貫通レイ上や敵の利きに入らないマスへの移動を生成する。
2. **王手駒の捕獲**：玉以外の味方駒で `checker_sq` に移動できる手を生成する。
3. **合い駒（移動合いと打つ合い）**：王手駒が飛び駒（香、角、飛、馬、龍）で玉と隣接していない場合、幾何テーブル `target = Bitboard::between(checker_sq, king_sq)` がそのまま遮断有効マスの集合となる。味方駒の `target` への移動手、および持ち駒の `target` への駒打ちを生成する。

---

## 2. 王手生成（`checks.rs`）と打ち駒優先ストリーミング

相手玉に王手をかける手だけを生成する `generate_checks` / `generate_checks_all` は、静止探索の王手生成や 1 手詰めソルバの中核を担う。
敵玉の位置を `enemy_king` とすると、王手になる手は **直接王手（Direct Check）** と **空き王手（Discovered Check）** の 2 経路から生成される。

### 直接王手と空き王手のビットボード抽出

1. **直接王手（移動後の駒自身が敵玉を睨む）**：`StateHot` に事前計算されている 9 スロットの [`CheckSquares`](../position/state-management.md) を参照する。
   駒種 `pt` の移動先候補と `check_squares(pt)` の論理積をとれば、そのまま不成での直接王手マスが得られる。
   さらに、成れる駒については移動元または移動先が敵陣（`promo_zone`）に含まれ、かつ移動先が `check_squares(promoted_pt)` に含まれる手を成りの直接王手として生成する。
2. **空き王手（味方駒が退くことで背後の味方飛び駒の利きが敵玉に通る）**：`StateHot` の `blockers_for_king(them)` には、「敵玉と味方の飛び駒の間を 1 枚だけで塞いでいる駒」がすでに記録されている。
   このうち味方の駒（`blockers_for_king(them) & our_pieces`）が、敵玉と移動元を結ぶ直線（`Bitboard::line(from, enemy_king)`）の外へ移動すれば、移動後の駒種にかかわらず必ず空き王手が成立する。
   直接王手と重複しないよう、直接王手にならなかった移動先のうち `!Bitboard::is_aligned(from, to, enemy_king)` を満たす手を空き王手として追加する。

### `generate_checks_all_move32_drop_first_into`（打ち駒優先生成）

1 手詰めソルバ（`mate.rs`）では、盤上の駒を動かす王手よりも **持ち駒を敵玉の周囲に打つ王手** の方が圧倒的に詰みに直結しやすい。
そのため `checks.rs` は、通常の生成順序（盤上移動 → 駒打ち）とは逆に、**まず持ち駒による王手打ち（`generate_drop_checks`）をシンクへ流し込み、それで決着しなかった場合にのみ盤上移動の王手（`generate_piece_checks`）を生成する** `generate_checks_all_move32_drop_first_into` を提供している。

---

## 3. 二歩と打ち歩詰め（`is_pawn_drop_mate`）の判定

歩を打つ手（`DROP_PAWN`）を生成および検証する際には、将棋特有の 2 つの禁手検査が行われる。

### 二歩のビットマスク一括除外

先手または後手の盤上の歩のビットボード `our_pawns = pos.pieces(us, PieceType::PAWN)` を用いて、歩が存在する筋をビット演算で特定する。
63+18 分割された File-major レイアウトにより、各筋の 9 ビット（`0x1ff << (file * 9)`）と `our_pawns` の交差を調べるだけで、すでに味方の歩がある筋の空きマスを歩打ち候補から一括でマスク除外できる。

以下の実戦局面では、2 筋と 8 筋以外のすべての筋に先手の歩（赤丸）がすでに存在するため、歩を打てる候補マス（黄色ハイライト）が 2 筋と 8 筋の空きマス（二段目以南）だけに絞り込まれる様子を示している。

<div id="live-nifu-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script src="../../assets/shogi-board.js"></script>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-nifu-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: true });
  board.setPositionFromSFEN('lnsgk1snl/6gb1/p1pppp2p/6pR1/9/1rP6/P2PPPP1P/1BG6/LNS1KGSNL b 2P2p 1');
  board.goTo(0);
  board.highlightSquares([
    (2 - 1) * 9 + (3 - 1),
    (2 - 1) * 9 + (5 - 1),
    (2 - 1) * 9 + (6 - 1),
    (2 - 1) * 9 + (7 - 1),
    (2 - 1) * 9 + (8 - 1),
    (8 - 1) * 9 + (3 - 1),
    (8 - 1) * 9 + (4 - 1),
    (8 - 1) * 9 + (5 - 1),
    (8 - 1) * 9 + (7 - 1),
  ]);
  board.setCircles([
    { square: '1g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '3g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '4g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '5g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '6g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '7f', color: 'rgba(220,50,50,0.7)', width: 1.5 },
    { square: '9g', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  window.liveNifu1 = board;
}
</script>

### 打ち歩詰めの盤面非破壊判定（`is_pawn_drop_mate`）

歩を打つマス `to` が敵玉のちょうど 1 マス前方（すなわち歩打ちによる王手）である場合、その手で敵玉が詰んでしまうと **打ち歩詰め（反則）** となる。
`crates/rsshogi/src/board/position/legality.rs` の `is_pawn_drop_mate(color, to)` は、`apply_move` で実際に局面を進めることなく、以下の 2 段階で高速に打ち歩詰めを判定する。

1. **玉以外の敵駒による歩の捕獲可能性**：打たれた歩のマス `to` に利いている敵駒を列挙し、敵玉を除いた候補 `non_king_capturers` を得る。
   これらのうち、敵玉に対して絶対ピンされていない駒（`!pinned.contains(from)`）、またはピンされていても移動方向がピンの直線に沿っている駒（`Bitboard::is_aligned(from, to, enemy_king)`）が **1 枚でも存在すれば**、敵はその駒で打たれた歩を合法に取れるため打ち歩詰めではない（`false` で即終了）。
2. **敵玉自身の脱出または歩の捕獲可能性**：歩が打たれた後の占有状態 `occupied_after = self.occupied() | Bitboard::from_square(to)` を仮想的に作る。
   敵玉の周囲 8 マスのうち敵の味方駒がないマス `king_escape_candidates`（歩が打たれた `to` 自身も含む）を走査し、`!self.is_square_attacked_with_occupied(sq, color, occupied_after)`（攻め方の利きがないマス）が **1 マスでも存在すれば**、敵玉はそのマスへ逃げられる（または打たれた歩を玉で取れる）ため打ち歩詰めではない（`false`）。
   どちらの応手も 1 つも存在しない場合にのみ、打ち歩詰め（`true`）と確定する。

以下の局面では、先手が `1四`（`1d`）に歩を打つと `1三`（`1c`）の後手玉に王手がかかるが、後手は玉以外の駒で `1四` の歩を取ることも、玉自身でその歩を取ったり周囲へ逃げたりすることもできないため、`is_pawn_drop_mate` によって打ち歩詰め（非合法手）と判定される。

<div id="live-uchifuzume-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-uchifuzume-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setOptions({ showHands: true });
  board.setPositionFromSFEN('l1+R2+R3/6ggl/p3ppppk/2p1b4/6S2/2P+b3N1/P+p3PPP1/4G1SK1/1N3+p1N1 b Pg2sn2l4p 1');
  board.goTo(0);
  board.setArrows([
    { from: 'S:P', to: '1d', color: 'rgba(220,50,50,0.7)', width: 1.5 },
  ]);
  board.setCircles([
    { square: '1c', color: 'rgba(220,50,50,0.8)', width: 2.0 },
  ]);
  window.liveUchifuzume1 = board;
}
</script>

---

## 4. 擬似合法性（`is_pseudo_legal`）と完全合法性（`is_legal`）の検証

置換表（TT）や Killer Move テーブルから取り出した指し手は、指し手生成を経由せずに直接検査されるため、まず `is_pseudo_legal_move32` で現局面の盤面、持ち駒、二歩、行き所のない駒、および王手中の整合性を検証する。
その上で、擬似合法手から真の合法手を絞り込む `is_legal_move32` は、以下の 3 点のみを検査する。

1. **打ち歩詰めの検査**：歩打ちのとき、`is_pawn_drop_mate(us, to)` が真なら非合法とする（駒打ちで自玉が新たに素抜かれることはないため、歩以外の駒打ちは常に合法となる）。
2. **玉自身の移動の検査**：動かす駒が玉（`KING`）のとき、移動元 `from` を空けた占有状態 `occupied ^ from` において移動先 `to` に敵の利きがあれば自殺手として非合法とする。
3. **絶対ピンされた駒の移動検査**：玉以外の盤上駒を動かすとき、移動元 `from` が `blockers_for_king(us)`（絶対ピン集合）に含まれていなければ無条件で合法となる。含まれている場合は、`Bitboard::is_aligned(from, to, our_king)` が真（ピンされている直線上の移動またはピン元の敵駒の捕獲）であるときのみ合法となる。

以下の盤面では、`5二` の後手飛車が `5五` の先手玉を睨んでおり、その間にある `5三` の先手銀（オレンジ丸）が `blockers_for_king(BLACK)` に登録されている。
この銀は 5 筋の直線上（`5二` の飛車を取る手、または `5四` へ引く手）にしか動けず、斜め方向（`4二`、`6二`、`4四`、`6四`）への移動は `!Bitboard::is_aligned(from, to, our_king)` により非合法として弾かれる。

<div id="live-pin-1" style="width:640px;height:720px;margin:1rem auto;border:1px solid #ddd;"></div>
<script>
{
  const { ShogiBoardAdapter } = RShogiBoard.installShogiBoardGlobals(window);
  const root = document.getElementById('live-pin-1');
  const board = new ShogiBoardAdapter();
  board.mount(root);
  board.setPositionFromSFEN('9/4r4/4S4/9/4K4/9/9/9/9 b - 1');
  board.goTo(0);
  board.setArrows([
    { from: '5b', to: '5e', color: 'rgba(220,50,50,0.6)', width: 1.2 },
  ]);
  board.setCircles([
    { square: '5c', color: 'rgba(255,140,0,0.8)', width: 2.0 },
  ]);
  window.livePin1 = board;
}
</script>

---

## 5. 入玉宣言勝ち判定（`evaluate_declaration`）

`crates/rsshogi/src/board/position/rules.rs` の `Position::evaluate_declaration(rule: EnteringKingRule)` は、コンピュータ将棋選手権（CSA ルール）等の入玉宣言勝ち条件をビットボード演算で高速判定する。

| ルール種別（`EnteringKingRule`） | 成立条件 |
| :--- | :--- |
| `NoEnteringKing` | 入玉宣言なし（常に不成立）。 |
| `Cs24Point` / `Cs27Point` | 1. 手番側の玉が敵陣三段目以内に入っていること。<br>2. 手番側の玉に王手がかかっていないこと。<br>3. 敵陣三段目以内にある「玉を除く自分の駒」が **10 枚以上** あること。<br>4. 敵陣内の自駒と自分の持ち駒の合計点数（大駒 5 点、小駒 1 点）が基準点（`Cs24Point` は 24 点以上、`Cs27Point` は先手 28 点、後手 27 点以上）を満たすこと。 |
| `Cs27PointHandicap` | 駒落ち対局用。上記 `Cs27Point` の必要点数に、初期局面で落ちている駒の点数を加算して判定する。 |
| `TryRule` | トライルール。手番側の玉に王手がかかっておらず、敵陣の初期玉位置（先手なら `5一`、後手なら `5九`）に自玉が到達していれば即座に勝ちとなる。 |
