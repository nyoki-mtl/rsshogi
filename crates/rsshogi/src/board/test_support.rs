use crate::board::Position;
use crate::types::{Move, Move32};

/// USI文字列から`Move32`を生成する。
///
/// - 成功した場合は`Some(Move)`を返す。
/// - 解析に失敗した場合は`None`を返す。
#[must_use]
pub fn move_from_usi(pos: &Position, usi: &str) -> Option<Move32> {
    let mv16 = Move::from_usi(usi)?;

    if mv16.is_drop() {
        let piece_type = mv16.dropped_piece()?;
        Some(Move32::drop(piece_type, mv16.to_sq(), pos.turn()))
    } else {
        let from = mv16.from_sq();
        let to = mv16.to_sq();
        let piece = pos.piece_on(from);

        if mv16.is_promotion() {
            Some(Move32::promotion(from, to, piece))
        } else {
            Some(Move32::normal(from, to, piece))
        }
    }
}

/// `move_from_usi` の `expect` 版。
#[must_use]
pub fn move_from_usi_expect(pos: &Position, usi: &str) -> Move32 {
    move_from_usi(pos, usi).unwrap_or_else(|| panic!("invalid USI move: {usi}"))
}

/// USI手を適用する。
///
/// - 適用した`Move32`を返す。
pub fn apply_usi_move(pos: &mut Position, usi: &str) -> Move32 {
    let mv = move_from_usi(pos, usi).unwrap_or_else(|| panic!("invalid USI move: {usi}"));
    debug_assert!(pos.is_legal_move32(mv), "apply_usi_move expects a legal move");
    pos.apply_move32(mv);
    mv
}

/// `apply_usi_move` の `expect` 版。
pub fn apply_usi_move_expect(pos: &mut Position, usi: &str) -> Move32 {
    apply_usi_move(pos, usi)
}
