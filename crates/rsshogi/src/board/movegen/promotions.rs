use crate::types::{Color, PieceType, Rank, Square};

/// 任意成り判定: 敵陣に入る/出る際に成れるか
#[inline]
pub(super) fn can_promote_move(from: Square, to: Square, us: Color) -> bool {
    let from_rank = from.rank().raw();
    let to_rank = to.rank().raw();

    match us {
        Color::BLACK => from_rank <= 2 || to_rank <= 2, // 先手は1-3段
        Color::WHITE => from_rank >= 6 || to_rank >= 6, // 後手は7-9段
    }
}

/// 不成りが禁止される行き所のない移動か判定
#[inline]
pub(super) fn must_promote(piece_type: PieceType, to: Square, us: Color) -> bool {
    match piece_type {
        PieceType::PAWN | PieceType::LANCE => match us {
            Color::BLACK => to.rank() == Rank::RANK_1,
            Color::WHITE => to.rank() == Rank::RANK_9,
        },
        PieceType::KNIGHT => match us {
            Color::BLACK => {
                let rank = to.rank();
                rank == Rank::RANK_1 || rank == Rank::RANK_2
            }
            Color::WHITE => {
                let rank = to.rank();
                rank == Rank::RANK_9 || rank == Rank::RANK_8
            }
        },
        _ => false,
    }
}
