use crate::board::{MoveList, Position};
use crate::types::{Bitboard, Color, Square};

use super::checks::generate_checks_for_color;
use super::drops::generate_drops_color;
use super::evasions::generate_evasions_for_color;
use super::pieces::{
    generate_br_moves, generate_gold_hdk_moves, generate_knight_moves, generate_lance_moves,
    generate_pawn_moves, generate_silver_moves,
};
use super::types::{Evasions, EvasionsAll, MoveGenType, NonEvasionsAll};
use super::{Black, ColorMarker, MoveSink, White};

/// 指し手を生成する（メインインターフェース）。
///
/// # Pseudo-Legal 手生成
///
/// この関数は基本的に pseudo-legal（擬似合法手）を生成する。生成段階では
/// ピン判定や自殺手の完全チェックを行わず、探索中に split legality API で
/// 遅延検証する設計とする。
///
/// 生成された手は次を含む可能性がある。
/// - ピンされた駒の違法な移動。
/// - 自殺手（玉を取られる手）。
/// - 打ち歩詰め（王手になる歩打ちのみ `is_legal_drop()` 相当で除外）。
///
/// これらは `Position::is_legal_after_pseudo_move32()` 相当で検証される前提とする。
#[inline]
// ANCHOR: movegen_generate_signature
pub fn generate_moves<T: MoveGenType + 'static>(pos: &Position, list: &mut MoveList) {
    generate_moves_into::<T, _>(pos, list);
}
// ANCHOR_END: movegen_generate_signature

/// 任意の [`MoveSink`] へ指し手を生成する。
///
/// [`generate_moves`] の汎用 sink 版。生成する手の意味は [`generate_moves`] と同じ。
#[allow(clippy::too_many_lines)]
#[inline]
pub fn generate_moves_into<T: MoveGenType + 'static, S: MoveSink>(pos: &Position, sink: &mut S) {
    match pos.turn() {
        Color::BLACK => generate_moves_color::<T, Black>(pos, sink),
        Color::WHITE => generate_moves_color::<T, White>(pos, sink),
    }
}

/// 全合法手を生成する。
///
/// - 王手中: `EVASIONS_ALL`
/// - 非王手: `NON_EVASIONS_ALL`
/// - 最後に `pos.is_legal_move32()` でフィルタ
///
/// `Move32` リストとして直接生成するには [`super::generate_legal_all_move32`] を使用。
pub fn generate_legal_all(pos: &Position, list: &mut MoveList) {
    generate_legal_all_into(pos, list);
}

/// 任意の [`MoveSink`] へ全合法手を生成する。
///
/// [`generate_legal_all`] の汎用 sink 版。
pub fn generate_legal_all_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    match pos.turn() {
        Color::BLACK => generate_legal_all_for_color::<Black>(pos, sink),
        Color::WHITE => generate_legal_all_for_color::<White>(pos, sink),
    }
}

/// 王手回避の legal-only 手を生成する。
///
/// 王手中の局面で、王手を回避する合法手だけを生成する。通常省略される不成は含まない。
/// すべての不成を含めたい場合は [`generate_legal_evasions_all`] を使う。
/// 王手されていない局面で呼び出してはならない。
pub fn generate_legal_evasions(pos: &Position, list: &mut MoveList) {
    generate_legal_evasions_into(pos, list);
}

/// 任意の [`MoveSink`] へ王手回避の legal-only 手を生成する。
///
/// [`generate_legal_evasions`] の汎用 sink 版。
pub fn generate_legal_evasions_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    debug_assert!(!pos.checkers().is_empty(), "generate_legal_evasions_into expects check");
    match pos.turn() {
        Color::BLACK => generate_legal_evasions_for_color::<Evasions, Black>(pos, sink),
        Color::WHITE => generate_legal_evasions_for_color::<Evasions, White>(pos, sink),
    }
}

/// 王手回避の legal-only 手を生成する（通常省略される不成も含む）。
///
/// 王手されていない局面で呼び出してはならない。
pub fn generate_legal_evasions_all(pos: &Position, list: &mut MoveList) {
    generate_legal_evasions_all_into(pos, list);
}

/// 任意の [`MoveSink`] へ王手回避の legal-only 手を生成する（不成を省略しない）。
///
/// [`generate_legal_evasions_all`] の汎用 sink 版。
pub fn generate_legal_evasions_all_into<S: MoveSink>(pos: &Position, sink: &mut S) {
    debug_assert!(!pos.checkers().is_empty(), "generate_legal_evasions_all_into expects check");
    match pos.turn() {
        Color::BLACK => generate_legal_evasions_for_color::<EvasionsAll, Black>(pos, sink),
        Color::WHITE => generate_legal_evasions_for_color::<EvasionsAll, White>(pos, sink),
    }
}

#[inline]
fn generate_legal_evasions_for_color<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    sink: &mut impl MoveSink,
) {
    generate_evasions_for_color::<T, C>(pos, sink);
    retain_generated_legal_for_color::<C>(pos, sink);
}

#[inline]
fn generate_legal_all_for_color<C: ColorMarker>(pos: &Position, list: &mut impl MoveSink) {
    if !pos.checkers().is_empty() {
        generate_evasions_for_color::<EvasionsAll, C>(pos, list);
    } else {
        generate_non_evasions_all_for_color::<C>(pos, list);
    }
    retain_generated_legal_for_color::<C>(pos, list);
}

/// `NON_EVASIONS_ALL` 相当の疑似合法手を生成する。
///
/// `LEGAL_ALL` のホットパス専用に、`generate_moves_color::<NonEvasionsAll, _>` の
/// 分岐を外した形で保持する。
#[inline(never)]
pub(super) fn generate_non_evasions_all_for_color<C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
) {
    let us = C::COLOR;
    let bb = pos.bitboards();
    let our_pieces = bb.color_pieces(us);
    let occupied = bb.occupied();
    let target = !our_pieces;
    let pawn_target = target;

    generate_pawn_moves::<NonEvasionsAll, C>(pos, list, pawn_target);
    generate_lance_moves::<NonEvasionsAll, C>(pos, list, target, occupied);
    generate_knight_moves::<NonEvasionsAll, C>(pos, list, target);
    generate_silver_moves::<NonEvasionsAll, C>(pos, list, target);
    generate_br_moves::<NonEvasionsAll, C>(pos, list, target, occupied);
    generate_gold_hdk_moves::<NonEvasionsAll, C>(pos, list, target, occupied);
    generate_drops_color::<NonEvasionsAll, C>(pos, list);
}

#[inline]
fn retain_generated_legal_for_color<C: ColorMarker>(pos: &Position, list: &mut impl MoveSink) {
    // LEGAL_ALL は「生成 → (必要なものだけ) 合法性フィルタ」で構成する。
    // `Position::is_legal_after_pseudo_move32()` は
    // 「玉の移動先が利かれていない」「ピン駒が直線上を動く」
    // だけを見れば十分なので、全手に対して `is_legal_move()` を呼ばない高速パスを使う。
    let us = C::COLOR;
    let king_sq = pos.king_square(us);
    let blockers = pos.blockers_for_king(us);

    list.retain_unordered(|mv16| {
        // 駒打ちは、自玉の安全性を悪化させない（占有が増えるだけ）ため常に合法。
        if mv16.is_drop() {
            return true;
        }

        let from = mv16.from_sq();
        if from == king_sq {
            // 玉の手だけは「移動先が利かれていない」を要チェック。
            // `Move -> Move -> split legality` の変換を避け、直接判定する（ホットパス）。
            if king_sq.is_none() {
                return true;
            }
            return if C::IS_BLACK {
                !pos.is_attacked_by_color_with_king_for::<false>(mv16.to_sq(), king_sq)
            } else {
                !pos.is_attacked_by_color_with_king_for::<true>(mv16.to_sq(), king_sq)
            };
        }

        // ピンされていない駒の移動は常に合法（自殺手にならない）。
        if !blockers.test(from) {
            return true;
        }

        // ピン駒は玉と同一直線上だけ動ける。
        if king_sq.is_none() {
            return true;
        }
        Bitboard::is_aligned(from, mv16.to_sq(), king_sq)
    });
}

#[allow(clippy::too_many_lines)]
#[inline]
fn generate_moves_color<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    list: &mut impl MoveSink,
) {
    use crate::types::Bitboard;
    let us = C::COLOR;
    let them = C::THEM;
    let bb = pos.bitboards();

    if T::IS_RECAPTURES {
        return;
    }

    let is_checks = T::IS_CHECKS;
    let is_legal = T::IS_LEGAL;

    if T::QUIET_CHECKS && !T::CAPTURES && !T::QUIETS {
        let all = T::is_generate_all_legal();
        generate_checks_for_color::<C>(pos, list, all, true);
        if !pos.checkers().is_empty() {
            list.retain_unordered(|m| pos.is_pseudo_legal_move(m, all));
        }
        return;
    }

    if is_checks {
        let all = T::is_generate_all_legal();
        generate_checks_for_color::<C>(pos, list, all, false);
        if !pos.checkers().is_empty() {
            list.retain_unordered(|m| pos.is_pseudo_legal_move(m, all));
        }
        return;
    }

    if T::EVASIONS {
        debug_assert!(T::CAPTURES && T::QUIETS, "EVASIONS must generate both captures and quiets.");
        generate_evasions_for_color::<T, C>(pos, list);
        if is_legal {
            retain_generated_legal_for_color::<C>(pos, list);
        }
        return;
    }

    if is_legal && !pos.checkers().is_empty() {
        if T::is_generate_all_legal() {
            generate_evasions_for_color::<EvasionsAll, C>(pos, list);
        } else {
            generate_evasions_for_color::<Evasions, C>(pos, list);
        }
        retain_generated_legal_for_color::<C>(pos, list);
        return;
    }

    // 自分の駒のBitboard
    let our_pieces = bb.color_pieces(us);
    // 相手の駒のBitboard
    let their_pieces = bb.color_pieces(them);
    // 全占有
    let occupied = bb.occupied();

    // ターゲット: 移動可能なマス（空マスまたは相手の駒）
    let target = if T::CAPTURES && T::QUIETS {
        !our_pieces // 自分の駒以外
    } else if T::CAPTURES {
        their_pieces // 相手の駒のみ
    } else if T::QUIETS {
        !occupied // 空マスのみ
    } else {
        return; // 何も生成しない
    };
    let enemy_territory = Bitboard::promotion_zone(us);
    let pawn_target = if T::IS_QUIETS_PRO_MINUS {
        (!occupied).and_not(enemy_territory)
    } else if T::IS_CAPTURE_PLUS_PRO {
        enemy_territory.and_not(our_pieces) | their_pieces
    } else {
        target
    };
    // 互換性のため、歩→香→桂→銀→角飛→金・馬・龍・玉の順で生成する。
    generate_pawn_moves::<T, C>(pos, list, pawn_target);
    generate_lance_moves::<T, C>(pos, list, target, occupied);
    generate_knight_moves::<T, C>(pos, list, target);
    generate_silver_moves::<T, C>(pos, list, target);
    generate_br_moves::<T, C>(pos, list, target, occupied); // YaneuraOu の GPM_BR に相当
    generate_gold_hdk_moves::<T, C>(pos, list, target, occupied); // YaneuraOu の GPM_GHDK に相当

    // 打ち駒生成
    if T::QUIETS {
        generate_drops_color::<T, C>(pos, list);
    }

    if is_legal {
        retain_generated_legal_for_color::<C>(pos, list);
    }
}

/// 指定マスへの移動手（Recaptures）を生成する。
#[inline]
pub fn generate_moves_to<T: MoveGenType + 'static>(
    pos: &Position,
    target_sq: Square,
    list: &mut MoveList,
) {
    generate_moves_to_into::<T, _>(pos, target_sq, list);
}

/// 任意の [`MoveSink`] へ指定マスへの移動手を生成する。
///
/// [`generate_moves_to`] の汎用 sink 版。
#[allow(clippy::too_many_lines)]
#[inline]
pub fn generate_moves_to_into<T: MoveGenType + 'static, S: MoveSink>(
    pos: &Position,
    target_sq: Square,
    sink: &mut S,
) {
    match pos.turn() {
        Color::BLACK => generate_moves_to_color::<T, Black>(pos, target_sq, sink),
        Color::WHITE => generate_moves_to_color::<T, White>(pos, target_sq, sink),
    }
}

#[allow(clippy::too_many_lines)]
#[inline]
fn generate_moves_to_color<T: MoveGenType + 'static, C: ColorMarker>(
    pos: &Position,
    target_sq: Square,
    list: &mut impl MoveSink,
) {
    use crate::types::Bitboard;

    debug_assert!(T::IS_RECAPTURES, "generate_moves_to is for Recaptures only");

    let us = C::COLOR;
    let bb = pos.bitboards();

    let is_legal = T::IS_LEGAL;

    if target_sq.is_none() {
        return;
    }

    // 全占有
    let occupied = bb.occupied();

    let target = Bitboard::from_square(target_sq);
    let enemy_territory = Bitboard::promotion_zone(us);
    let pawn_target = if T::IS_QUIETS_PRO_MINUS {
        (!occupied).and_not(enemy_territory)
    } else if T::IS_CAPTURE_PLUS_PRO {
        enemy_territory.and_not(bb.color_pieces(us)) | bb.color_pieces(us.flip())
    } else {
        target
    };

    // 生成順は generate_moves_color と同じ。
    generate_pawn_moves::<T, C>(pos, list, pawn_target);
    generate_lance_moves::<T, C>(pos, list, target, occupied);
    generate_knight_moves::<T, C>(pos, list, target);
    generate_silver_moves::<T, C>(pos, list, target);
    generate_br_moves::<T, C>(pos, list, target, occupied);
    generate_gold_hdk_moves::<T, C>(pos, list, target, occupied);

    if is_legal {
        retain_generated_legal_for_color::<C>(pos, list);
    }
}
