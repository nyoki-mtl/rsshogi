//! 定跡I/O。Native Book v4、外部reader、変換用の所有IRを提供する。
//!
//! Native形式の候補手参照は[`NativeBook`]、編集は[`NativeBookEditor`]を使う。

mod control;
mod database;
mod error;
mod native;
mod patch;
mod sbk;
mod writer;
mod ybb;
mod yaneuraou;

pub use control::BookControl;
pub use database::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata,
    BookPosition, SbkEntryMetadata, SbkEvalMetadata, SbkMoveMetadata, YaneuraOuEntryMetadata,
    YaneuraOuMoveMetadata,
};
pub use error::BookError;
pub use native::*;
pub use patch::{BookDatabaseEdit, BookDatabasePatch, BookPatchWritePhase, BookPatchWriteProgress};
pub use sbk::{
    SbkBook, SbkDiagnostics, SbkEntries, SbkEntry, SbkEval, SbkMove, SbkOpenProgress,
    SbkWriteOptions,
};
pub use writer::{YaneuraOuDb2016WriteOptions, YaneuraOuDb2016Writer};
pub use yaneuraou::{
    YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookDiagnostics, YaneuraOuBookEntries,
    YaneuraOuBookEntry, YaneuraOuBookMove, YaneuraOuBookOpenOptions, YaneuraOuValidationProgress,
};
pub use ybb::{
    YbbBook, YbbBookOpenOptions, YbbDiagnostics, YbbEntries, YbbEntry, YbbMatchingEntries, YbbMove,
    YbbStorageMode, YbbValidationMode, YbbValidationProgress, YbbWriteOptions, YbbWriter,
};
