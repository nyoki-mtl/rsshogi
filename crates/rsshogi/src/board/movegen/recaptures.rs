use crate::board::{Move32List, MoveList, Position};
use crate::types::Square;

use super::types::{Recaptures, RecapturesAll};
use super::{generate::generate_moves_to, generate_moves_to_move32};

/// 指定マスへの移動手（Recaptures）を生成
pub fn generate_recaptures(pos: &Position, target: Square, list: &mut MoveList) {
    generate_moves_to::<Recaptures>(pos, target, list);
}

/// 指定マスへの移動手（Recaptures, 歩の不成なども含む）を生成
pub fn generate_recaptures_all(pos: &Position, target: Square, list: &mut MoveList) {
    generate_moves_to::<RecapturesAll>(pos, target, list);
}

/// 指定マスへの移動手（Recaptures）を Move32 リストとして生成
pub fn generate_recaptures_move32(pos: &Position, target: Square, list: &mut Move32List) {
    generate_moves_to_move32::<Recaptures>(pos, target, list);
}

/// 指定マスへの移動手（Recaptures, 歩の不成なども含む）を Move32 リストとして生成
pub fn generate_recaptures_all_move32(pos: &Position, target: Square, list: &mut Move32List) {
    generate_moves_to_move32::<RecapturesAll>(pos, target, list);
}
