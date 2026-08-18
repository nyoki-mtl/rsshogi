//! 一手詰め判定。
//!
//! 判定の骨格は「詰みの否定（= 相手の合法応手の存在証明）を apply なしで示す」構造を採る。
//! 王手を受けた側の合法応手は次の 3 種に限られる（王手放置は非合法で、応手が玉を動かさず
//! 王手駒も取らず王手線も遮断しないなら、王手駒の利きは応手後も玉に届き続けるため）。
//!
//! - (a) 玉の移動（捕獲を含む）
//! - (b) 玉以外の駒による王手駒の捕獲
//! - (c) 王手線（王手駒と玉の間の升）への移動または打ち（合駒）
//!
//! 各否定フィルタは「合法応手を 1 つ構成的に示せた場合のみ」候補を棄却する。示せない場合は
//! 必ず生存させ、生存候補は従来どおり apply + [`Position::is_mated`] で確定する。この一方向
//! 設計により、フィルタの誤りが結果へ現れうるのは「棄却の誤り = 詰み見逃し」だけであり、
//! 誤詰み（false positive）は構造的に発生しない。棄却の健全性は debug ビルドの候補単位
//! オラクル（棄却候補を apply して非詰みを assert）で常時検査する。

use crate::board::attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS, bishop_attacks,
    lance_attacks, rook_attacks,
};
use crate::board::movegen::{
    Move32Sink, generate_checks_all_move32, generate_checks_all_move32_drop_first_into,
};
use crate::board::{Move32List, Ply, Position, generate_legal_all_move32};
use crate::types::{Bitboard, Color, Hand, Move32, Piece, PieceType, Rank, Square};

/// 一手詰めは clone 後に 1 手だけ適用し、反復判定を使わないため、複製する履歴を限定する。
const MATE_WORK_HISTORY_PLY: Ply = 16;

/// 相手玉の周囲を、局面ごとに1回だけ構造化したもの。
///
/// 詰みであるためには相手玉の移動先がすべて塞がっている必要がある。これは必要条件なので、
/// 満たさない候補は指し手を適用せずに捨てられる。候補ごとの判定はビット演算だけで済み、
/// 利きの再計算をしない。
///
/// 判定は必ず「詰みかもしれない」側へ倒す。取りこぼしを防ぐため、
/// 見積もりが曖昧になる場合は候補を通し、従来どおり適用して確定させる。
struct KingNeighborhood {
    /// 玉が動ける升（逃げ道升）。相手駒の居る隣接升は含まない。
    squares: [Square; 8],
    /// `squares` の各升を攻撃している手番側の駒（玉抜き占有基準）。
    attackers: [Bitboard; 8],
    /// `squares` の各升に対して、空けると飛び利きが開きうる升。
    /// ここから駒が退く手は、その升だけ適用後の利きを再計算して確定する。
    discovery: [Bitboard; 8],
    len: usize,
    /// 玉に隣接する相手の駒。候補がここを捕獲すると新しい逃げ道になりうるため、
    /// 捕獲後の利きを候補ごとに 1 回だけ再計算して確定する（升単位の事前計算は
    /// しない。捕獲候補の数は隣接相手駒の升数より少ないのが普通で、再計算のほうが
    /// 利き問い合わせの総数が少ない）。
    adjacent_their_pieces: Bitboard,
    /// 玉を取り除いた占有。玉の背後へ利きが伸びることを正しく扱うために使う。
    occupied_without_king: Bitboard,
}

/// `piece` が `sq` から利かせる升を返す。
fn piece_attacks(piece: Piece, sq: Square, occupied: Bitboard) -> Bitboard {
    let color = piece.color();
    match piece.piece_type() {
        PieceType::PAWN => PAWN_ATTACKS[sq][color.to_index()],
        PieceType::LANCE => lance_attacks(sq, occupied, color),
        PieceType::KNIGHT => KNIGHT_ATTACKS[sq][color.to_index()],
        PieceType::SILVER => SILVER_ATTACKS[sq][color.to_index()],
        PieceType::GOLD
        | PieceType::PRO_PAWN
        | PieceType::PRO_LANCE
        | PieceType::PRO_KNIGHT
        | PieceType::PRO_SILVER => GOLD_ATTACKS[sq][color.to_index()],
        PieceType::BISHOP => bishop_attacks(sq, occupied),
        PieceType::ROOK => rook_attacks(sq, occupied),
        PieceType::HORSE => bishop_attacks(sq, occupied) | KING_ATTACKS[sq],
        PieceType::DRAGON => rook_attacks(sq, occupied) | KING_ATTACKS[sq],
        PieceType::KING => KING_ATTACKS[sq],
        _ => Bitboard::ALL,
    }
}

/// 駒台から打った駒が `to` から利かせる升を返す。
fn dropped_piece_attacks(
    piece_type: PieceType,
    to: Square,
    us: Color,
    occupied: Bitboard,
) -> Bitboard {
    piece_attacks(Piece::from_parts(us, piece_type), to, occupied)
}

fn king_neighborhood(position: &Position) -> Option<KingNeighborhood> {
    let us = position.turn();
    let them = us.flip();
    let king = position.king_square(them);
    if king.is_none() {
        return None;
    }

    let bitboards = position.bitboards();
    let their_pieces = bitboards.color_pieces(them);
    let occupied_without_king = bitboards.occupied().and_not(Bitboard::from_square(king));
    let adjacent = KING_ATTACKS[king];
    let adjacent_their_pieces = adjacent & their_pieces;

    let mut squares = [Square::NONE; 8];
    let mut attackers = [Bitboard::EMPTY; 8];
    let mut discovery = [Bitboard::EMPTY; 8];
    let mut len = 0;

    // 自軍の飛び駒。これらと隣接升の間にある升を空けると利きが開く。
    let sliders =
        (bitboards.bishop_horse() | bitboards.rook_dragon() | bitboards.pieces(PieceType::LANCE))
            & bitboards.color_pieces(us);

    let mut remaining = adjacent.and_not(their_pieces);
    while let Some(square) = remaining.pop_lsb() {
        squares[len] = square;
        attackers[len] = position.attackers_to_color(us, square, occupied_without_king);

        // 逃げ道へ利きが通りうる飛び駒だけを見て、その間の升を集める。
        // ここから駒が退く手は、その升だけ適用後の利きを再計算して確定する。
        let mut opened = Bitboard::EMPTY;
        let mut candidates = sliders;
        while let Some(slider) = candidates.pop_lsb() {
            let piece = position.piece_on(slider);
            if piece_attacks(piece, slider, Bitboard::EMPTY).test(square) {
                opened |= Bitboard::between(slider, square);
            }
        }
        discovery[len] = opened;
        len += 1;
    }

    Some(KingNeighborhood {
        squares,
        attackers,
        discovery,
        len,
        adjacent_their_pieces,
        occupied_without_king,
    })
}

impl KingNeighborhood {
    /// 駒打ちの候補が玉の逃げ道を塞ぎ切らないなら `true`。
    fn drop_leaves_escape(&self, piece_type: PieceType, to: Square, us: Color) -> bool {
        let occupied = self.occupied_without_king | Bitboard::from_square(to);
        let attacks = dropped_piece_attacks(piece_type, to, us, occupied);
        for index in 0..self.len {
            let square = self.squares[index];
            if attacks.test(square) || !self.attackers[index].is_empty() {
                continue;
            }
            return true;
        }
        false
    }

    /// `square` への攻め方の利きを、候補適用後の玉抜き占有で 1 回だけ再計算する。
    ///
    /// 逃げ道判定の対象升は玉自身が退く先なので、玉の背後へ飛び利きが伸びることを
    /// 正しく扱うために玉抜き占有を使う。駒集合の staleness 補正は
    /// [`attackers_with_delta`] に一元化されたものをそのまま使うため、結果は
    /// 「適用後にその升を攻撃している手番側の駒」と正確に一致する。
    fn recomputed_attackers(
        &self,
        position: &Position,
        square: Square,
        from: Square,
        to: Square,
        piece_after: Piece,
        occupied_after: Bitboard,
    ) -> Bitboard {
        let delta = MoveDelta { from: Some(from), to, piece_after, occupied_after };
        attackers_with_delta(position, piece_after.color(), square, &delta)
    }

    /// 盤上の駒を動かす候補が玉の逃げ道を塞ぎ切らないなら `true`。
    fn board_move_leaves_escape(
        &self,
        position: &Position,
        from: Square,
        to: Square,
        piece_after: Piece,
    ) -> bool {
        let to_bit = Bitboard::from_square(to);
        let from_bit = Bitboard::from_square(from);
        let occupied = self.occupied_without_king.and_not(from_bit) | to_bit;

        // 玉に隣接する相手駒を取る手は、その升自体が新しい逃げ道候補になる。捕獲後の
        // `to` へ玉が取り返す手は、`to` に攻め方の利きが残っていない場合に限り合法に
        // なるので、その升だけ適用後の利きを 1 回再計算して確定する（移動駒自身は
        // 自分の居る升を攻撃しないため数に入らない）。
        if !(to_bit & self.adjacent_their_pieces).is_empty()
            && self.recomputed_attackers(position, to, from, to, piece_after, occupied).is_empty()
        {
            return true;
        }

        let attacks = piece_attacks(piece_after, to, occupied);
        for index in 0..self.len {
            let square = self.squares[index];
            if attacks.test(square) {
                continue;
            }
            // 動かす駒以外の利きが残っていれば塞がったまま。
            if !self.attackers[index].and_not(from_bit).is_empty() {
                continue;
            }
            // 元の升を空けることで新たな利きが通りうる升だけ適用後の利きを再計算し、
            // 封鎖が実際に残っているなら塞がったままと確定する。
            if self.discovery[index].test(from)
                && !self
                    .recomputed_attackers(position, square, from, to, piece_after, occupied)
                    .is_empty()
            {
                continue;
            }
            return true;
        }
        false
    }
}

/// 否定フィルタが共有する受け方情報。局面ごとに 1 回構築する。
struct MateContext {
    us: Color,
    /// 受け方玉の升。
    their_king: Square,
    their_king_bb: Bitboard,
    /// 受け方玉のブロッカー（`blockers_for_king` の cache）。
    ///
    /// cache は「障害物がちょうど 1 枚の線」しか含まないため、移動元退去による新規ピンは
    /// ここからは読めない。新規ピンは `is_aligned` の過大近似で「確実に合法」の側だけを狭める。
    their_blockers: Bitboard,
    /// 候補適用前の占有。
    occupied: Bitboard,
    /// 受け方の持ち駒。合駒を打てるかの判定に使う。
    their_hand: Hand,
    /// 攻め方（手番側）の玉の升。玉なし局面では `Square::NONE`。
    ///
    /// 歩の合駒打ちがこの玉へ王手をかける場合、その打ちは打ち歩詰め（非合法）の可能性を
    /// 排除できないため、棄却根拠にしない保守ガードに使う。
    our_king: Square,
}

fn mate_context(position: &Position) -> Option<MateContext> {
    let us = position.turn();
    let their_king = position.king_square(us.flip());
    if their_king.is_none() {
        return None;
    }
    Some(MateContext {
        us,
        their_king,
        their_king_bb: Bitboard::from_square(their_king),
        their_blockers: position.blockers_for_king(us.flip()),
        occupied: position.bitboards().occupied(),
        their_hand: position.hand(us.flip()),
        our_king: position.king_square(us),
    })
}

/// 候補手を適用せずに、適用後の利きを問い合わせるための差分情報。
///
/// `Position` の駒種 bitboard は候補適用前のまま参照するため、そのままでは移動元に古い
/// ビットが残り、捕獲された駒も盤上に居るように見える。飛び利きの遮断・開放は
/// `occupied_after` が正しく扱うので、駒集合側の補正は [`attackers_with_delta`] に一元化し、
/// フィルタ内で生の `attackers_to_color` を直接呼ぶことは禁止する。
///
/// `occupied_after` は問い合わせの文脈が要求する占有を持つ。捕獲・合駒の否定では
/// 「適用後の占有」そのもの、逃げ道の再計算では「適用後の占有から受け方玉を除いたもの」
/// を渡す（玉の背後へ飛び利きが伸びることを扱うため）。
#[derive(Clone, Copy)]
struct MoveDelta {
    /// 盤上移動の移動元。駒打ちなら `None`。
    from: Option<Square>,
    to: Square,
    /// 移動後の駒（成りを反映済み）。
    piece_after: Piece,
    /// 候補適用後の占有。
    occupied_after: Bitboard,
}

impl MoveDelta {
    fn from_move32(mv: Move32, occupied: Bitboard) -> Self {
        let to = mv.to_sq();
        let to_bit = Bitboard::from_square(to);
        let (from, occupied_after) = if mv.is_drop() {
            (None, occupied | to_bit)
        } else {
            let from = mv.from_sq();
            (Some(from), occupied.and_not(Bitboard::from_square(from)) | to_bit)
        };
        Self { from, to, piece_after: mv.piece_after_move(), occupied_after }
    }
}

/// 候補適用後の局面で `target` を攻撃する `color` の駒を、盤面を書き換えずに返す。
///
/// `attackers_to_color` を適用後占有で呼んだ結果へ、駒集合の staleness を次の順で補正する。
///
/// - 移動元 `from` のビットを消す（移動駒の古い位置。打ちなら補正なし）。
/// - `to` のビットを消す。適用後の `to` は攻め方の移動駒が占有しており、
///   捕獲された受け方駒の古いビットが攻撃駒として混入するのを防ぐ。
/// - 攻め方側の問い合わせでは、移動駒が `to` から `target` へ利くならビットを立て直す。
///
/// 受け方の駒は候補では 1 枚も動かないため、上記以外の補正は不要。
fn attackers_with_delta(
    position: &Position,
    color: Color,
    target: Square,
    delta: &MoveDelta,
) -> Bitboard {
    let mut attackers = position.attackers_to_color(color, target, delta.occupied_after);
    if let Some(from) = delta.from {
        attackers.clear(from);
    }
    attackers.clear(delta.to);
    if color == delta.piece_after.color()
        && piece_attacks(delta.piece_after, delta.to, delta.occupied_after).test(target)
    {
        attackers.set(delta.to);
    }
    attackers
}

/// 候補手が与える王手の分類。
#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckClass {
    /// 移動駒（または打った駒）自身による王手。王手駒の升は移動先。
    Direct(Square),
    /// 開き王手。王手駒の升は移動元の背後に居た飛び駒の升。
    Discovered(Square),
    /// 両王手。2 本の王手線は玉だけを共有し 1 手で同時に遮断も捕獲もできないため、
    /// 合法応手は玉移動のみになり、捕獲・合駒の否定は適用しない。
    Double,
}

/// 候補手の王手を分類する。
///
/// `gives_check_move32` と同じ規則（直接王手 = `check_square`、開き王手 = ブロッカー退去
/// かつ非直線移動）で構成する。開き王手の王手駒が特定できない場合は `None` を返し、
/// 呼び出し側は候補を生存させる。
fn classify_check(position: &Position, ctx: &MateContext, mv: Move32) -> Option<CheckClass> {
    let to = mv.to_sq();
    if mv.is_drop() {
        return Some(CheckClass::Direct(to));
    }
    let from = mv.from_sq();
    let moved_pt = mv.piece_after_move().piece_type();
    let direct = position.check_square(moved_pt).test(to);
    let discovered =
        ctx.their_blockers.test(from) && !Bitboard::is_aligned(from, to, ctx.their_king);
    match (direct, discovered) {
        (true, true) => Some(CheckClass::Double),
        (true, false) => Some(CheckClass::Direct(to)),
        (false, true) => discovered_checker_square(position, ctx, from).map(CheckClass::Discovered),
        // 王手候補の生成契約上ここには来ないが、分類できない場合は生存させる。
        (false, false) => None,
    }
}

/// 移動元 `from` の退去で開く王手の王手駒（攻め方の飛び駒）の升を返す。
///
/// 玉からの ray は升ごとに高々 1 本なので、`line(玉, from)` 上の攻め方飛び駒のうち
/// 「玉との間の障害物が `from` だけ」のものが唯一の該当駒になる。空盤利きの `test` で
/// 香の利き方向と駒種ごとの線種（斜め・縦横）も確認する。
fn discovered_checker_square(
    position: &Position,
    ctx: &MateContext,
    from: Square,
) -> Option<Square> {
    let bb = position.bitboards();
    let mut sliders = (bb.bishop_horse() | bb.rook_dragon() | bb.pieces(PieceType::LANCE))
        & bb.color_pieces(ctx.us)
        & Bitboard::line(ctx.their_king, from);
    let from_bit = Bitboard::from_square(from);
    while let Some(slider) = sliders.pop_lsb() {
        if (Bitboard::between(slider, ctx.their_king) & ctx.occupied) == from_bit
            && piece_attacks(position.piece_on(slider), slider, Bitboard::EMPTY)
                .test(ctx.their_king)
        {
            return Some(slider);
        }
    }
    None
}

/// 飛び利きを持つ駒種なら `true`。
fn is_slider(pt: PieceType) -> bool {
    matches!(
        pt,
        PieceType::LANCE
            | PieceType::BISHOP
            | PieceType::ROOK
            | PieceType::HORSE
            | PieceType::DRAGON
    )
}

/// 受け方の駒が `mover` から `target` へ確実に合法に動けるなら `true`。
///
/// 王手駒の捕獲 (b) と移動合駒 (c) が共有するピン判定。合法性は次の 3 条件で
/// 「確実に合法」の側だけを狭める。いずれも過大側の誤りは
/// 「合法応手を見逃す → 候補生存 → apply」で健全。
///
/// - 既存ピン: ブロッカー cache に載っていない、または移動が pin 線上
///   （玉からの ray は升ごとに高々 1 本なので `is_aligned` で足りる）。
///   王手駒の玉への線は遮断なしで通っており第 2 の線は存在しないため、王手駒自身が
///   受け方の駒の pinner になることはなく、cache から王手駒を除外する補正は不要。
/// - 移動元退去による新規ピン: cache は障害 1 枚の線しか持たないため、
///   `is_aligned(受け方の駒, 移動元, 玉)` の過大近似で除外する。
/// - 移動駒が新たな pinner になる場合（`to_may_pin`）: 開き王手かつ移動駒が飛び駒の
///   ときに限る（直接王手では移動駒の升から玉への線は王手線ただ 1 本で、その上に
///   受け方の駒は居ないため pinner になりえない）。同じく過大近似で除外する。
///
/// 歩・香・桂による最奥段への移動は、最奥段が受け方の敵陣内にあり成る指し手が常に
/// 合法に存在するため、行き所チェックは不要。
fn surely_legal_defender_move(
    ctx: &MateContext,
    delta: &MoveDelta,
    to_may_pin: bool,
    mover: Square,
    target: Square,
) -> bool {
    if ctx.their_blockers.test(mover) && !Bitboard::is_aligned(mover, target, ctx.their_king) {
        return false;
    }
    if let Some(from) = delta.from
        && Bitboard::is_aligned(mover, from, ctx.their_king)
    {
        return false;
    }
    if to_may_pin && Bitboard::is_aligned(mover, delta.to, ctx.their_king) {
        return false;
    }
    true
}

/// 受け方の玉以外の駒が王手駒を確実に合法に捕獲できるなら `true`（応手 (b) の否定）。
///
/// 単一王手だけを対象にし、捕獲を 1 つ構成的に示せた場合のみ棄却根拠にする。玉による捕獲は
/// 逃げ道フィルタ (a) の担当なので数えない。合法性判定は [`surely_legal_defender_move`]。
fn refute_by_capture(
    position: &Position,
    ctx: &MateContext,
    delta: &MoveDelta,
    check: CheckClass,
) -> bool {
    let (checker, discovered) = match check {
        CheckClass::Direct(c) => (c, false),
        CheckClass::Discovered(c) => (c, true),
        CheckClass::Double => return false,
    };
    let them = ctx.us.flip();
    let mut capturers =
        attackers_with_delta(position, them, checker, delta).and_not(ctx.their_king_bb);
    let to_may_pin = discovered && is_slider(delta.piece_after.piece_type());
    while let Some(capturer) = capturers.pop_lsb() {
        if surely_legal_defender_move(ctx, delta, to_may_pin, capturer, checker) {
            return true;
        }
    }
    false
}

/// 受け方が王手線へ確実に合法な合駒を用意できるなら `true`（応手 (c) の否定）。
///
/// 単一王手かつ王手線 L = between(王手駒, 玉) が非空のときだけ対象になる（桂王手・
/// 隣接王手は L が空で自動的に対象外）。L の升は王手成立の定義により空である。
///
/// 打ち合駒は、駒打ちが占有を追加するだけで自玉の露出を起こさないことから、L 上の空升への
/// 打ちは (i) 二歩、(ii) 行き所のない駒（歩・香は受け方の最終段、桂は最終 2 段）、
/// (iii) その歩打ち自体が攻め方の玉を詰ます打ち歩詰めになる場合、を除いて常に合法であり
/// 王手を解消する。(iii) は「歩打ちが攻め方の玉へ王手をかけない」ことを条件にする
/// 保守ガードで排除する（王手をかけないなら詰みでもありえず打ち歩詰め規則の適用外）。
///
/// 二歩判定は候補適用前の受け方の歩 bitboard で行う。候補が受け方の歩を捕獲した場合に
/// stale になるが、誤り方向は「打てるのに打てないと判定 → 棄却しない → apply」で健全。
///
/// 移動合駒は L 上の各升へ利かせられる受け方の駒（将棋の駒は移動先と利き先が一致する）を
/// [`attackers_with_delta`] で列挙し、合法性判定は捕獲と同じ
/// [`surely_legal_defender_move`] を使う。受け方玉の L への移動は逃げ道フィルタ (a) の
/// 担当なので数えない。
fn refute_by_interpose(
    position: &Position,
    ctx: &MateContext,
    delta: &MoveDelta,
    check: CheckClass,
) -> bool {
    let (checker, discovered) = match check {
        CheckClass::Direct(c) => (c, false),
        CheckClass::Discovered(c) => (c, true),
        CheckClass::Double => return false,
    };
    let line = Bitboard::between(checker, ctx.their_king);
    if line.is_empty() {
        return false;
    }

    let them = ctx.us.flip();
    let hand = ctx.their_hand;

    // 打ち合駒。金銀角飛は段制限がなく無条件に合法な打ちが存在する。
    if hand.has(PieceType::GOLD)
        || hand.has(PieceType::SILVER)
        || hand.has(PieceType::BISHOP)
        || hand.has(PieceType::ROOK)
    {
        return true;
    }
    if hand.has(PieceType::LANCE) || hand.has(PieceType::KNIGHT) || hand.has(PieceType::PAWN) {
        // 受け方の駒が進めなくなる最終段。歩・香は最終段、桂は最終 2 段に打てない。
        let (last, second) = if them == Color::BLACK {
            (Rank::RANK_1, Rank::RANK_2)
        } else {
            (Rank::RANK_9, Rank::RANK_8)
        };
        let last_bb = Bitboard::rank_mask(last);
        if hand.has(PieceType::LANCE) && !line.and_not(last_bb).is_empty() {
            return true;
        }
        if hand.has(PieceType::KNIGHT)
            && !line.and_not(last_bb | Bitboard::rank_mask(second)).is_empty()
        {
            return true;
        }
        if hand.has(PieceType::PAWN) {
            let their_pawns = position.bitboards().pieces_for(PieceType::PAWN, them);
            let mut targets = line.and_not(last_bb);
            while let Some(target) = targets.pop_lsb() {
                if !(their_pawns & Bitboard::file_mask(target.file())).is_empty() {
                    continue;
                }
                if !ctx.our_king.is_none()
                    && PAWN_ATTACKS[target][them.to_index()].test(ctx.our_king)
                {
                    continue;
                }
                return true;
            }
        }
    }

    // 移動合駒。
    let to_may_pin = discovered && is_slider(delta.piece_after.piece_type());
    let mut targets = line;
    while let Some(target) = targets.pop_lsb() {
        let mut interposers =
            attackers_with_delta(position, them, target, delta).and_not(ctx.their_king_bb);
        while let Some(interposer) = interposers.pop_lsb() {
            if surely_legal_defender_move(ctx, delta, to_may_pin, interposer, target) {
                return true;
            }
        }
    }
    false
}

/// 静的フィルタが棄却した候補を適用し、非詰みであることを検査する（debug ビルド専用）。
///
/// 否定フィルタの誤りは「棄却の誤り = 詰み見逃し」の一方向にしか現れないため、棄却側だけを
/// apply で突き合わせれば健全性を検査できる。違反時は再現に必要な SFEN と候補の raw 値を
/// 出力する。
#[cfg(debug_assertions)]
fn assert_rejected_candidate_is_not_mate(position: &Position, mv: Move32) {
    let mut next = position.clone();
    next.init_stack();
    next.apply_move32_with_gives_check(mv, position.gives_check_move32(mv));
    assert!(
        !next.is_mated(),
        "static refutation filter rejected a mating move: sfen={} move_raw={:#010x} usi={}",
        position.to_sfen(None),
        mv.raw(),
        mv.to_usi(),
    );
}

/// 現局面での一手詰めを返す。
///
/// 非王手局面では打ち王手を盤上王手より先に走査し、最初の詰み確定で生成全体を停止する。
/// 作業局面は最初の静的反証生存候補で [`Position::clone_for_search`] して遅延作成する。
/// 王手中は board-first の候補列・判定経路を使う。生存候補が出た局面でも局面複製を避けたい
/// 呼び出し側は [`solve_mate_in_one_in_place`] を直接使う。
#[must_use]
pub fn solve_mate_in_one(position: &Position) -> Option<Move32> {
    if position.is_in_check() {
        return solve_mate_in_one_accepted(position);
    }

    let mut work = None;
    let mut sink = ImmutableMateSink { position, context: None, work: &mut work, found: None };
    generate_checks_all_move32_drop_first_into(position, &mut sink);
    sink.found
}

/// 王手中の immutable API が board-first の候補列・判定経路を使うための実装。
fn solve_mate_in_one_accepted(position: &Position) -> Option<Move32> {
    let mut candidates = Move32List::new();
    let prep = prepare_mate_candidates(position, &mut candidates)?;

    // 作業局面の clone + state stack 初期化はキー・キャッシュの全再計算を含み、候補 1 個の
    // apply より重い。全候補が静的に棄却される局面で丸ごと省けるよう、最初の生存候補まで
    // 遅延させる。
    let mut work: Option<Position> = None;

    for &mv in candidates.iter() {
        if prep.in_check && !position.gives_check_move32(mv) {
            continue;
        }
        if is_statically_refuted(position, &prep, mv) {
            continue;
        }
        let work = work.get_or_insert_with(|| {
            let mut cloned = position.clone();
            cloned.init_stack();
            cloned
        });
        work.apply_move32_with_gives_check(mv, true);
        if work.is_mated() {
            return Some(mv);
        }
        work.undo_move32(mv).expect("mate search must undo the move it just applied");
    }

    None
}

struct ImmutableMateSink<'a> {
    position: &'a Position,
    context: Option<MateFilterContext>,
    work: &'a mut Option<Position>,
    found: Option<Move32>,
}

impl Move32Sink for ImmutableMateSink<'_> {
    fn push_move32(&mut self, mv: Move32) {
        if self.stop() || !self.position.is_legal_move32(mv) {
            return;
        }
        if self.context.is_none() {
            self.context = Some(MateFilterContext {
                in_check: false,
                us: self.position.turn(),
                neighborhood: king_neighborhood(self.position),
                context: mate_context(self.position),
            });
        }
        let context = self.context.as_ref().expect("legal candidate must initialize mate context");
        if is_statically_refuted(self.position, context, mv) {
            return;
        }
        if self.work.is_none() {
            *self.work = Some(self.position.clone_for_search_bounded(MATE_WORK_HISTORY_PLY));
        }
        let work = self.work.as_mut().expect("mate work position must be initialized");
        work.apply_move32_with_gives_check(mv, true);
        let mated = work.is_mated();
        work.undo_move32(mv).expect("mate search must undo the move it just applied");
        if mated {
            self.found = Some(mv);
        }
    }

    fn retain_unordered<F>(&mut self, _: F)
    where
        F: FnMut(Move32) -> bool,
    {
        unreachable!("streaming check generation does not retain candidates")
    }

    fn stop(&self) -> bool {
        self.found.is_some()
    }
}

/// 現局面での一手詰めを、作業局面を複製せずに返す。
///
/// 非王手局面では王手候補を直接生成し、王手中では合法な回避手から候補を選ぶ。
/// 各候補はまず静的な否定フィルタ（逃げ道の存在・王手駒の捕獲・合駒）にかけ、
/// 相手の合法応手を構成できた候補は適用せずに捨てる。歩打ち候補は打ち歩詰め規則により
/// 詰みになりえないため、フィルタより前に捨てる。
/// 生き残った候補だけを `position` 自身へ適用し、防御側に合法手がなければ一手詰めである。
/// 適用した候補は判定後に必ず巻き戻すため、正常復帰時の `position` は呼び出し前と
/// 完全に一致する（debug ビルドでは出口で board key・持ち駒・手番・stack 深さを検査する）。
///
/// # 前提条件
///
/// `position` の state stack が現局面に同期して初期化済みであること
/// （[`Position::init_stack`] 済み、または SFEN 等からの構築直後）。debug ビルドでは
/// 入口でキャッシュ整合を検査する。
///
/// # Panics
///
/// 内部不変条件の違反（バグ）でのみ panic しうる。panic 後の `position` の状態は
/// 未定義であり、これは他の `&mut Position` 変異 API と同一の契約である。
#[must_use]
pub fn solve_mate_in_one_in_place(position: &mut Position) -> Option<Move32> {
    position.debug_assert_partial_keys_consistent();
    #[cfg(debug_assertions)]
    let entry = (
        position.board_key(),
        position.hand(Color::BLACK),
        position.hand(Color::WHITE),
        position.turn(),
        position.state_stack_depth(),
    );

    let result = solve_mate_in_one_impl(position);

    // undo 漏れ・undo 誤りの検出器。正常経路の出口では局面が入口と一致していなければ
    // ならない。
    #[cfg(debug_assertions)]
    debug_assert_eq!(
        entry,
        (
            position.board_key(),
            position.hand(Color::BLACK),
            position.hand(Color::WHITE),
            position.turn(),
            position.state_stack_depth(),
        ),
        "solve_mate_in_one_in_place must restore the position it mutated"
    );

    result
}

/// [`solve_mate_in_one_in_place`] の判定本体。生存候補を `position` 自身へ適用して確定する。
///
/// 詰みが見つかった場合も含めて適用した候補は必ず巻き戻すため、正常復帰時の `position` は
/// 呼び出し前と一致する。
fn solve_mate_in_one_impl(position: &mut Position) -> Option<Move32> {
    let mut candidates = Move32List::new();
    let prep = prepare_mate_candidates(position, &mut candidates)?;

    for &mv in candidates.iter() {
        if prep.in_check && !position.gives_check_move32(mv) {
            continue;
        }
        if is_statically_refuted(position, &prep, mv) {
            continue;
        }
        position.apply_move32_with_gives_check(mv, true);
        let mated = position.is_mated();
        position.undo_move32(mv).expect("mate search must undo the move it just applied");
        if mated {
            return Some(mv);
        }
    }

    None
}

/// 両 API の駆動ループが共有する、局面ごとに 1 回構築する受け方情報。
struct MateFilterContext {
    /// 手番側が王手を受けているか。王手中は合法回避手から候補を選ぶ。
    in_check: bool,
    us: Color,
    neighborhood: Option<KingNeighborhood>,
    context: Option<MateContext>,
}

/// `candidates` へ候補列を生成し、受け方情報を構築する。候補が 1 つも無ければ `None`。
fn prepare_mate_candidates(
    position: &Position,
    candidates: &mut Move32List,
) -> Option<MateFilterContext> {
    let in_check = position.is_in_check();
    if in_check {
        generate_legal_all_move32(position, candidates);
    } else {
        generate_checks_all_move32(position, candidates);
        candidates.retain_unordered(|mv| position.is_legal_move32(mv));
    }

    if candidates.is_empty() {
        return None;
    }

    let us = position.turn();
    let neighborhood = king_neighborhood(position);
    // 王手中の候補選別は従来経路のまま残し、捕獲の否定は非王手局面だけに適用する。
    let context = if in_check { None } else { mate_context(position) };

    Some(MateFilterContext { in_check, us, neighborhood, context })
}

/// 候補が静的な否定フィルタで棄却されるなら `true`（適用せずに捨ててよい）。
///
/// 判定ロジックの単一の真実点。両 API の駆動ループは、この述語を生き残った候補だけを
/// 作業局面へ適用して確定し、最初の詰み確定で走査ごと打ち切る。棄却時は debug ビルドの
/// 候補単位オラクルで非詰みを検査する。
#[inline]
fn is_statically_refuted(position: &Position, prep: &MateFilterContext, mv: Move32) -> bool {
    // 歩を打って相手玉を詰ませる手は打ち歩詰めで非合法なので、一手詰めの解に歩打ちは
    // 存在しない。候補生成側は打ち歩詰めを既に除外しており、候補列に残る歩打ちは
    // すべて非詰みである（生成側の除外は他用途があるため触らない）。
    if !prep.in_check && mv.dropped_piece() == Some(PieceType::PAWN) {
        #[cfg(debug_assertions)]
        assert_rejected_candidate_is_not_mate(position, mv);
        return true;
    }

    if let Some(neighborhood) = prep.neighborhood.as_ref() {
        let leaves_escape = match mv.dropped_piece() {
            Some(piece_type) => neighborhood.drop_leaves_escape(piece_type, mv.to_sq(), prep.us),
            None => neighborhood.board_move_leaves_escape(
                position,
                mv.from_sq(),
                mv.to_sq(),
                mv.piece_after_move(),
            ),
        };
        if leaves_escape {
            #[cfg(debug_assertions)]
            assert_rejected_candidate_is_not_mate(position, mv);
            return true;
        }
    }

    if let Some(ctx) = prep.context.as_ref()
        && let Some(check) = classify_check(position, ctx, mv)
        && check != CheckClass::Double
    {
        let delta = MoveDelta::from_move32(mv, ctx.occupied);
        if refute_by_capture(position, ctx, &delta, check) {
            #[cfg(debug_assertions)]
            assert_rejected_candidate_is_not_mate(position, mv);
            return true;
        }
        if refute_by_interpose(position, ctx, &delta, check) {
            #[cfg(debug_assertions)]
            assert_rejected_candidate_is_not_mate(position, mv);
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests;
