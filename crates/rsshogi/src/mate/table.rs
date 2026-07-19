//! テーブル駆動の 1 手詰め判定。
//!
//! `mate_constant.rs` で事前計算される 65,536 エントリのテーブルを使用し、
//! 玉の周囲 8 方向の利き状況をインデックス化して詰み判定を行う。

use crate::board::attack_tables::{
    GOLD_ATTACKS, KING_ATTACKS, KNIGHT_ATTACKS, PAWN_ATTACKS, SILVER_ATTACKS, bishop_attacks,
    lance_attacks, rook_attacks,
};
use crate::board::mate_constant::mate_info;
use crate::board::movegen::generate_checks;
use crate::board::{MoveList, Position};
use crate::types::Bitboard;
use crate::types::{Color, HandPiece, Move32, Piece, PieceType, Square};
const OFFSETS_AROUND8: [(i8, i8); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// center の周囲 8 方向のうち盤内にある升のビットマスクを返す。
fn valid_around8(center: Square) -> u8 {
    let mut mask = 0u8;
    let (cx, cy) = (center.file().raw(), center.rank().raw());
    for (i, &(dx, dy)) in OFFSETS_AROUND8.iter().enumerate() {
        let nx = cx + dx;
        let ny = cy + dy;
        if (0..9).contains(&nx) && (0..9).contains(&ny) {
            mask |= 1 << i;
        }
    }
    mask
}

/// `center` の周囲 8 方向について `bb` に含まれる升のビットマスクを返す。
fn around8(bb: Bitboard, center: Square) -> u8 {
    let mut mask = 0u8;
    let (cx, cy) = (center.file().raw(), center.rank().raw());

    for (i, &(dx, dy)) in OFFSETS_AROUND8.iter().enumerate() {
        let nx = cx + dx;
        let ny = cy + dy;
        if (0..9).contains(&nx) && (0..9).contains(&ny) {
            #[allow(clippy::cast_sign_loss)]
            let idx = (nx * 9 + ny) as usize;
            let n_sq = Square::from_index(idx);
            if bb.test(n_sq) {
                mask |= 1 << i;
            }
        }
    }
    mask
}

/// `color` の駒が利きを持つ、玉周囲 8 升のビットマスクを返す。
fn around8_attacks(pos: &Position, center: Square, color: Color) -> u8 {
    let mut mask = 0u8;
    let (cx, cy) = (center.file().raw(), center.rank().raw());
    for (i, &(dx, dy)) in OFFSETS_AROUND8.iter().enumerate() {
        let nx = cx + dx;
        let ny = cy + dy;
        if (0..9).contains(&nx) && (0..9).contains(&ny) {
            #[allow(clippy::cast_sign_loss)]
            let idx = (nx * 9 + ny) as usize;
            let n_sq = Square::from_index(idx);
            if (pos.attackers_to(n_sq, pos.bitboards().occupied())
                & pos.bitboards().color_pieces(color))
            .any()
            {
                mask |= 1 << i;
            }
        }
    }
    mask
}

/// `occupied` の盤面占有を前提に、`sq` が `attacker_color` の駒に利かれているか判定する。
fn is_attacked(
    pos: &Position,
    sq: Square,
    occupied: Bitboard,
    attacker_color: Color,
    moved_piece: Option<(Option<Square>, Square, PieceType)>, // (移動元, 移動先, 駒種)
) -> bool {
    let attackers = attackers_to_with_moved(pos, sq, occupied, attacker_color, moved_piece);
    !attackers.is_empty()
}

fn attackers_to_with_moved(
    pos: &Position,
    sq: Square,
    occupied: Bitboard,
    attacker_color: Color,
    moved_piece: Option<(Option<Square>, Square, PieceType)>, // (移動元, 移動先, 駒種)
) -> Bitboard {
    let mut attackers =
        pos.attackers_to(sq, occupied) & pos.bitboards().color_pieces(attacker_color);
    let moved_color = pos.turn();

    if attacker_color != moved_color {
        return attackers;
    }

    if let Some((from_opt, to, pt)) = moved_piece {
        if let Some(from) = from_opt {
            attackers.clear(from);
        }

        let attacks_from_to = piece_attacks(pt, to, attacker_color, occupied);
        if attacks_from_to.test(sq) {
            attackers.set(to);
        }
    }

    attackers
}

/// 指定した駒種、手番、升、盤面占有に対応する利きビットボードを返す。
fn piece_attacks(pt: PieceType, sq: Square, color: Color, occupied: Bitboard) -> Bitboard {
    match pt {
        PieceType::PAWN => PAWN_ATTACKS[sq][color.to_index()],
        PieceType::LANCE => lance_attacks(sq, occupied, color),
        PieceType::KNIGHT => KNIGHT_ATTACKS[sq][color.to_index()],
        PieceType::SILVER => SILVER_ATTACKS[sq][color.to_index()],
        PieceType::GOLD
        | PieceType::PRO_PAWN
        | PieceType::PRO_LANCE
        | PieceType::PRO_KNIGHT
        | PieceType::PRO_SILVER => GOLD_ATTACKS[sq][color.to_index()],
        PieceType::KING => KING_ATTACKS[sq],
        PieceType::BISHOP => bishop_attacks(sq, occupied),
        PieceType::ROOK => rook_attacks(sq, occupied),
        PieceType::HORSE => bishop_attacks(sq, occupied) | KING_ATTACKS[sq],
        PieceType::DRAGON => rook_attacks(sq, occupied) | KING_ATTACKS[sq],
        _ => Bitboard::EMPTY,
    }
}

/// `sq` にある駒が玉 `king_sq` に対して絶対ピンされているか判定する。
fn is_pinned(
    pos: &Position,
    sq: Square,
    king_sq: Square,
    occupied: Bitboard,
    my_color: Color,
    moved_piece: Option<(Option<Square>, Square, PieceType)>, // (移動元, 移動先, 駒種)
) -> bool {
    let enemy_color = my_color.flip();

    // 同列、同段、斜め方向のみピンあり得る

    let direction = sq.file() == king_sq.file() || sq.rank() == king_sq.rank() || {
        let dx = i32::from(sq.file().raw()) - i32::from(king_sq.file().raw());
        let dy = i32::from(sq.rank().raw()) - i32::from(king_sq.rank().raw());
        dx.abs() == dy.abs()
    };

    if !direction {
        return false;
    }

    let occupied_without_pin = occupied ^ Bitboard::from_square(sq);
    let real_attackers = attackers_to_with_moved(pos, king_sq, occupied, enemy_color, moved_piece);
    let simulated_attackers =
        attackers_to_with_moved(pos, king_sq, occupied_without_pin, enemy_color, moved_piece);

    let exposed = simulated_attackers & !real_attackers;

    !exposed.is_empty()
}

/// 候補手 `mv` が詰みを成立させるか検証する。
fn is_mate_bitboard(pos: &Position, mv: Move32) -> bool {
    let us = pos.turn();
    let them = us.flip();
    let king_sq =
        pos.bitboards().pieces_for(PieceType::KING, them).lsb().expect("opponent must have a king");

    let mut occupied = pos.bitboards().occupied();
    let moves_from = mv.from_sq();
    let moves_to = mv.to_sq();
    let captured_piece = if mv.is_drop() {
        None
    } else {
        let captured = pos.piece_on(moves_to);
        if captured == Piece::NONE { None } else { Some((captured, moves_to)) }
    };

    let moved_piece_def = if mv.is_drop() {
        occupied.set(moves_to);
        let pt = mv.dropped_piece().expect("drop move must have dropped piece");
        Some((None, moves_to, pt))
    } else {
        occupied.clear(moves_from);
        occupied.set(moves_to);
        let mut pt = pos.piece_on(moves_from).piece_type();
        if mv.is_promotion() {
            pt = pt.promote();
        }
        Some((Some(moves_from), moves_to, pt))
    };

    // 指し手後の盤面で王手駒を再計算する。
    // `moves_to` だけが王手駒とは限らない（開き王手の可能性がある）。
    let mut actual_checkers = Bitboard::from_square(moves_to);

    // 1. 移動した駒による直接王手か？
    let pt_at_dest = match moved_piece_def {
        Some((_, _, pt)) => pt,
        None => pos.piece_on(moves_to).piece_type(),
    };

    // 移動先の駒が玉に利いているか確認する
    let attacks = piece_attacks(pt_at_dest, moves_to, us, occupied);
    if !attacks.test(king_sq) {
        actual_checkers.clear(moves_to);
    }

    // 2. 開き王手の検出
    // 指し手後の `occupied` で自陣側の王手駒を全列挙する。
    // `pos.attackers_to` は `pos` のビットボードを参照するため `from` がまだ占有扱いだが、
    // `occupied` 引数が飛び利きの遮蔽を正しく処理する。
    // `from` の駒が攻撃者として検出される場合のみ補正が必要。
    let mut discovered = pos.attackers_to(king_sq, occupied) & pos.bitboards().color_pieces(us);
    if let Some((Some(from), _, _)) = moved_piece_def {
        discovered.clear(from);
    }

    actual_checkers |= discovered;

    if actual_checkers.is_empty() {
        return false;
    }

    let double_check = actual_checkers.count() > 1;

    // 2. 玉の逃げ道を確認する
    let king_moves = KING_ATTACKS[king_sq];
    let mut them_pieces = pos.bitboards().color_pieces(them);
    // 取られた駒を後手の駒マスクから除く（取り逃げ判定の誤りを防ぐ）
    them_pieces.clear(moves_to);

    let escape_targets = king_moves & !them_pieces;

    let mut targets_iter = escape_targets;
    while let Some(dest) = targets_iter.pop_lsb() {
        let occupied_for_escape = occupied ^ Bitboard::from_square(king_sq);
        if !is_attacked(pos, dest, occupied_for_escape, us, moved_piece_def) {
            return false;
        }
    }

    if double_check {
        return true;
    }

    // 一枚王手の対処（王手駒を取る / 合い駒を打つ）
    let checker_sq = actual_checkers.lsb().expect("single check must have checker");

    // 3. 王手駒を取れるか確認する
    let mut potential_capturers = pos.attackers_to(checker_sq, occupied) & them_pieces;
    potential_capturers.clear(king_sq);

    while let Some(cap_sq) = potential_capturers.pop_lsb() {
        if is_pinned(pos, cap_sq, king_sq, occupied, them, moved_piece_def) {
            let ray = Bitboard::line(king_sq, cap_sq);
            if ray.test(checker_sq) {
                return false;
            }
            continue;
        }
        return false;
    }

    // 4. 合い駒を打てるか確認する
    // 直接王手と開き王手のいずれも玉と王手駒の間の経路を正確に使う。
    let path = Bitboard::between(king_sq, checker_sq);

    if !path.is_empty() {
        let mut path_iter = path;
        while let Some(sq) = path_iter.pop_lsb() {
            let mut blockers = pos.attackers_to(sq, occupied) & them_pieces;
            blockers.clear(king_sq);

            let hand = pos.hand(them);
            if hand.bits() > 0 && has_valid_drop(pos, sq, them, hand, captured_piece) {
                return false;
            }

            while let Some(from) = blockers.pop_lsb() {
                if is_pinned(pos, from, king_sq, occupied, them, moved_piece_def) {
                    let ray = Bitboard::line(king_sq, from);
                    if ray.test(sq) {
                        return false;
                    }
                    continue;
                }
                return false;
            }
        }
    }

    true
}

/// 持ち駒から `sq` へ合法的に打てる駒が存在するか判定する。
fn has_valid_drop(
    pos: &Position,
    sq: Square,
    color: Color,
    hand: crate::types::Hand,
    captured_piece: Option<(Piece, Square)>,
) -> bool {
    let has_non_pawn = hand.count(HandPiece::LANCE) > 0
        || hand.count(HandPiece::KNIGHT) > 0
        || hand.count(HandPiece::SILVER) > 0
        || hand.count(HandPiece::GOLD) > 0
        || hand.count(HandPiece::BISHOP) > 0
        || hand.count(HandPiece::ROOK) > 0;

    if has_non_pawn {
        let rank = sq.rank().raw();
        if color == Color::BLACK {
            if rank == 0 {
                if hand.count(HandPiece::SILVER) > 0
                    || hand.count(HandPiece::GOLD) > 0
                    || hand.count(HandPiece::BISHOP) > 0
                    || hand.count(HandPiece::ROOK) > 0
                {
                    return true;
                }
                return false;
            }
            if rank == 1 {
                if hand.count(HandPiece::LANCE) > 0
                    || hand.count(HandPiece::SILVER) > 0
                    || hand.count(HandPiece::GOLD) > 0
                    || hand.count(HandPiece::BISHOP) > 0
                    || hand.count(HandPiece::ROOK) > 0
                {
                    return true;
                }
                return false;
            }
        }
        if color == Color::WHITE {
            if rank == 8 {
                if hand.count(HandPiece::SILVER) > 0
                    || hand.count(HandPiece::GOLD) > 0
                    || hand.count(HandPiece::BISHOP) > 0
                    || hand.count(HandPiece::ROOK) > 0
                {
                    return true;
                }
                return false;
            }
            if rank == 7 {
                if hand.count(HandPiece::LANCE) > 0
                    || hand.count(HandPiece::SILVER) > 0
                    || hand.count(HandPiece::GOLD) > 0
                    || hand.count(HandPiece::BISHOP) > 0
                    || hand.count(HandPiece::ROOK) > 0
                {
                    return true;
                }
                return false;
            }
        }
        return true;
    }

    if hand.count(HandPiece::PAWN) > 0
        && !has_pawn_on_file_after_capture(pos, color, sq.file(), captured_piece)
    {
        if (color == Color::BLACK && sq.rank().raw() == 0)
            || (color == Color::WHITE && sq.rank().raw() == 8)
        {
            return false;
        }
        return true;
    }

    false
}

fn has_pawn_on_file_after_capture(
    pos: &Position,
    color: Color,
    file: crate::types::File,
    captured_piece: Option<(Piece, Square)>,
) -> bool {
    let mut pawns = pos.bitboards().pieces_for(PieceType::PAWN, color);
    if let Some((piece, sq)) = captured_piece
        && piece.piece_type() == PieceType::PAWN
        && piece.color() == color
    {
        pawns.clear(sq);
    }
    !(pawns & Bitboard::file_mask(file)).is_empty()
}

/// 現局面で先手側が一手で詰ませられる指し手を探索する。
/// 一手詰め判定。
#[must_use]
#[allow(clippy::too_many_lines)]
pub(super) fn solve_mate_in_one_table(pos: &Position) -> Option<Move32> {
    let us = pos.turn();
    let them = us.flip();

    if pos.bitboards().pieces_for(PieceType::KING, them).is_empty() {
        return None;
    }
    let ksq = pos.bitboards().pieces_for(PieceType::KING, them).lsb().expect("king not found");

    // 1. テーブル参照用インデックスを構築する
    let occupied_bb = pos.bitboards().occupied();
    let valid_mask = valid_around8(ksq);
    let them_pieces = pos.bitboards().color_pieces(them);
    let a8_them_movable = (!around8(them_pieces, ksq)) & valid_mask;
    let a8_targetable = (!around8(pos.bitboards().color_pieces(us), ksq)) & valid_mask;
    let a8_attacks_us = around8_attacks(pos, ksq, us);

    let info1 = a8_targetable;
    let info2 = a8_them_movable & !a8_attacks_us;
    let idx = (usize::from(info1)) | (usize::from(info2) << 8);
    let c = us.to_index();

    let mate_info = mate_info(idx, c);

    // 2. 駒打ち詰めの確認
    if mate_info.hand_kind != 0 || mate_info.directions != 0 {
        let hand = pos.hand(us);
        let needed = mate_info.hand_kind;
        if needed != 0 {
            let types = [
                (PieceType::GOLD, 6),
                (PieceType::SILVER, 3),
                (PieceType::ROOK, 5),
                (PieceType::LANCE, 1),
                (PieceType::BISHOP, 4),
                (PieceType::KNIGHT, 2),
            ];

            for &(pt, bit_idx) in &types {
                if (needed & (1 << bit_idx)) != 0
                    && hand.count(HandPiece::from_piece_type(pt).expect("valid hand piece type"))
                        > 0
                {
                    for dir in 0..8 {
                        if (valid_mask & (1 << dir)) != 0 {
                            let (cx, cy) = (ksq.file().raw(), ksq.rank().raw());
                            let offsets = [
                                (0, -1),
                                (1, -1),
                                (1, 0),
                                (1, 1),
                                (0, 1),
                                (-1, 1),
                                (-1, 0),
                                (-1, -1),
                            ];
                            let (dx, dy) = offsets[dir];
                            let nx = i32::from(cx) + dx;
                            let ny = i32::from(cy) + dy;

                            #[allow(clippy::cast_sign_loss)]
                            let idx2 = (nx * 9 + ny) as usize;
                            let to = Square::from_index(idx2);

                            if !occupied_bb.test(to) {
                                // テーブルの圧縮により王手にならない手が候補に挙がることがあるため、実際に王手かを確認する
                                let new_occ = occupied_bb | Bitboard::from_square(to);
                                if !piece_attacks(pt, to, us, new_occ).test(ksq) {
                                    continue;
                                }

                                let mv = Move32::drop(pt, to, us);
                                if is_mate_bitboard(pos, mv) {
                                    return Some(mv);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. 駒移動による詰みの確認
    let mut checks = MoveList::new();
    generate_checks(pos, &mut checks);

    for mv in checks.iter() {
        let mv_move = pos.move32_from_move(*mv);
        if !mv_move.is_normal() || !pos.is_legal_move32(mv_move) {
            continue;
        }

        if is_mate_bitboard(pos, mv_move) {
            return Some(mv_move);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::movegen::{NonEvasionsAll, generate_moves};

    const MATE_TEST_SFENS: &[&str] = &[
        "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
        "ln2+P2nl/2R1+S1g2/p2p1p1p+B/8p/5+R3/2p3PkP/PP1PPP3/2+bS1KS2/5G1NL b GL4Pgsn 83",
        "l2+R3g1/2ln5/2k1ps+Bp1/2p3P2/p3Sp1P1/7b1/PLPKPP3/1S1G2G2/LN1s+n+r1N1 b G4P3p 103",
        "l2+S3kl/9/3p1pG+S1/ppp3Pp1/4+RP1np/P1n1S1p2/1P1PS1gN1/2G1G4/L2K3RL b 3P2bn2p 115",
        "l1ggk3l/3plsG2/4sp+P2/p2+Bp2pp/5n1+b1/2+RSPN3/Pp1P1P+n1P/3K5/L7+n b RS3Pg2p 117",
    ];

    #[test]
    fn test_detects_mate_in_one_move() {
        for &sfen in MATE_TEST_SFENS {
            let position = crate::board::position_from_sfen(sfen).expect("valid sfen");

            let mv = solve_mate_in_one_table(&position).expect("mate move should exist");

            // 詰みを局面を進めて検証する
            let mut next = position.clone();
            next.init_stack();

            next.apply_move32(mv);
            assert!(!next.checkers().is_empty(), "resulting position must give check");

            let mut replies = MoveList::new();
            generate_moves::<NonEvasionsAll>(&next, &mut replies);

            let mut legals = 0;
            for r in replies.iter() {
                if next.is_legal_move(*r) {
                    legals += 1;
                }
            }
            assert_eq!(legals, 0, "opponent should have no legal replies");
        }
    }

    #[test]
    fn test_returns_none_when_no_mate() {
        let position = crate::board::hirate_position();
        assert!(solve_mate_in_one_table(&position).is_none());
    }
}
