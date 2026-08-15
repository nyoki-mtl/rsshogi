#![cfg(any(feature = "book", feature = "position-serialization", feature = "records"))]

#[cfg(feature = "book")]
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(feature = "position-serialization")]
use rsshogi::board::{PackedSfen, Position};
#[cfg(feature = "book")]
use rsshogi::book::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata,
    BookPosition, YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions,
    YaneuraOuDb2016WriteOptions, YbbBook,
};
#[cfg(feature = "records")]
use rsshogi::records::formats::pack::{PackError, PackGameResult, decode_game};
#[cfg(feature = "book")]
use rsshogi::types::Move;

#[cfg(feature = "book")]
fn temp_path(suffix: &str) -> PathBuf {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).expect("clock after epoch").as_nanos();
    std::env::temp_dir()
        .join(format!("rsshogi-task0026-malformed-{}-{nonce}-{suffix}", std::process::id()))
}

#[cfg(feature = "position-serialization")]
#[test]
fn all_zero_packed_sfen_is_an_error_and_never_panics() {
    let packed = PackedSfen::from_bytes([0; 32]);
    let result = std::panic::catch_unwind(|| {
        let mut position = Position::empty();
        position.set_packed_sfen(&packed, false, 0)
    });
    assert!(result.is_ok(), "malformed PackedSfen must not panic");
    assert!(result.expect("catch result").is_err());
}

#[cfg(feature = "records")]
#[test]
fn pack_rejects_structurally_invalid_raw_moves() {
    for raw in [0x0204_u16, 0xc081_u16, 0x6880_u16, 0x2c00_u16] {
        let bytes = [1, raw as u8, (raw >> 8) as u8, 0, 0, 0, 0, 0];
        assert!(decode_game(&bytes).is_err(), "raw {raw:#06x} must be rejected");
    }
}

#[cfg(feature = "records")]
#[test]
fn pack_accepts_structurally_valid_apery_promotion() {
    let bytes = [1, 0x80, 0x48, 0, 0, 0x81, 0, 0];
    let (game, consumed) = decode_game(&bytes).expect("Apery promotion must decode");
    assert_eq!(game.plies[0].mv.raw(), 0x4880);
    assert_eq!(consumed, bytes.len());
}

#[cfg(feature = "records")]
#[test]
fn pack_rejects_inconsistent_terminal_outcome_during_decode() {
    assert!(matches!(
        decode_game(&[1, 0, 0, 0]),
        Err(PackError::InconsistentOutcome { result: PackGameResult::Draw, .. })
    ));
}

#[cfg(feature = "records")]
#[test]
fn pack_validates_hcp_start_payload_during_decode() {
    let mut bytes = vec![0];
    bytes.extend_from_slice(&[0xff; 32]);
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0x81_u16.to_le_bytes());
    bytes.push(0);
    assert!(decode_game(&bytes).is_err());
}

#[cfg(feature = "book")]
#[test]
fn db2016_reports_invalid_drop_piece_for_that_entry() {
    let path = temp_path("invalid.db");
    let text = "#YANEURAOU-DB2016 1.00\n\
sfen 4k4/9/9/9/9/9/9/9/4K4 b - 1\n\
K*5e none 0 0 0\n";
    fs::write(&path, text).expect("fixture write");
    let options = YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::ScanOnly);
    let book = YaneuraOuBook::open_with_options(&path, options).expect("open structural book");
    assert!(book.iter_entries().expect("iterator").next().expect("entry result").is_err());
    let _ = fs::remove_file(path);
}

#[cfg(feature = "book")]
#[test]
fn db2016_writer_rejects_noncanonical_raw_drop_source() {
    let position = BookPosition::from_position(&rsshogi::board::hirate_position(), Some(1));
    let candidate = BookCandidate::new(
        Move::from_raw(0x4880),
        None,
        None,
        None,
        None,
        None,
        BookMoveMetadata::new(),
    );
    let entry = BookDatabaseEntry::new(position, vec![candidate], BookEntryMetadata::new());
    let database = BookDatabase::try_from_entries(vec![entry]).expect("database");
    assert!(database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new()).is_err());
}

#[cfg(feature = "book")]
#[test]
fn db2016_unsorted_checked_prefix_cannot_cause_lookup_miss() {
    let path = temp_path("unsorted.db");
    let later = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let earlier = "4k4/9/9/9/9/9/9/9/4K4 b - 1";
    let text = format!("#YANEURAOU-DB2016 1.00\nsfen {later}\nsfen {earlier}\n");
    fs::write(&path, text).expect("fixture write");
    let options =
        YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedAfterPrefix {
            prefix_rows: 2,
        });
    let result =
        YaneuraOuBook::open_with_options(&path, options).and_then(|book| book.lookup_sfen(earlier));
    assert!(result.is_err() || result.expect("lookup result").is_some());
    let _ = fs::remove_file(path);
}

#[cfg(feature = "book")]
#[test]
fn ybb_rejects_same_square_board_move() {
    let path = temp_path("invalid.ybb");
    let position = rsshogi::board::hirate_position();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"YANE-BINBOOK-V1\0");
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(position.to_packed_sfen().as_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&position.game_ply().to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0x0204_u16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    fs::write(&path, bytes).expect("fixture write");
    assert!(YbbBook::open(&path).is_err());
    let _ = fs::remove_file(path);
}

#[cfg(feature = "book")]
#[test]
fn ybb_rejects_noncanonical_raw_drop_source() {
    let path = temp_path("noncanonical-drop.ybb");
    let position = rsshogi::board::hirate_position();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"YANE-BINBOOK-V1\0");
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(position.to_packed_sfen().as_bytes());
    bytes.extend_from_slice(&0_u64.to_le_bytes());
    bytes.extend_from_slice(&position.game_ply().to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0x4880_u16.to_le_bytes());
    bytes.extend_from_slice(&0_i16.to_le_bytes());
    fs::write(&path, bytes).expect("fixture write");
    assert!(YbbBook::open(&path).is_err());
    let _ = fs::remove_file(path);
}
