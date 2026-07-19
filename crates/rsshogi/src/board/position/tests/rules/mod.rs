pub(super) use crate::board::Position;
pub(super) use crate::types::Move32;
pub(super) use crate::types::Piece;

// declaration_win は駒落ち required-points を InitialPosition 経由で検証するため
// initial-positions feature が要る。
#[cfg(feature = "initial-positions")]
mod declaration_win;
mod discovered_checks;
mod drops;
mod helpers;
mod repetition;
mod split_legality;
