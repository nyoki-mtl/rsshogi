//! 自己対局教師データ用の sazpack (`SAZ2`)。

pub use super::sazpack_selfplay::*;

pub type SazError = SazSelfplayError;
pub type SazGame = SazSelfplayGame;
pub type SazPosition = SazSelfplayPosition;
pub type SazPolicyEntry = SazSelfplayPolicyEntry;

pub use deserialize_selfplay_chunk as deserialize_chunk;
pub use deserialize_selfplay_chunk_indexed as deserialize_chunk_indexed;
pub use serialize_selfplay_chunk as serialize_chunk;
