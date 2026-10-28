use std::io::Cursor;

use super::*;
use crate::book::sbk::{scan_sbook_bytes, write_varint};

fn field(number: u32, bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::new();
    write_varint(&mut output, u64::from(number << 3 | 2));
    write_varint(&mut output, bytes.len() as u64);
    output.extend_from_slice(bytes);
    output
}

fn scan_result(bytes: &[u8], streaming: bool) -> String {
    let result =
        if streaming { scan_file(&mut Cursor::new(bytes)) } else { scan_sbook_bytes(bytes) };
    format!("{result:?}")
}

#[test]
fn file_scan_matches_slice_scan_and_truncations() {
    let mut bytes = field(FIELD_SBOOK_AUTHOR, "著者".as_bytes());
    bytes.extend(field(FIELD_SBOOK_STATE, &[8, 37, 8, 42, 0xff]));
    bytes.extend(field(100, &[0xff; 1024]));
    bytes.extend(field(FIELD_SBOOK_DESCRIPTION, b"description"));
    bytes.extend(field(FIELD_SBOOK_STATE, &[16, 1]));
    bytes.extend([0xa1, 6]);
    bytes.extend([0; 8]);
    bytes.extend([0xad, 6]);
    bytes.extend([0; 4]);
    bytes.extend(field(FIELD_SBOOK_AUTHOR, b"last author"));
    for end in 0..=bytes.len() {
        assert_eq!(
            scan_result(&bytes[..end], true),
            scan_result(&bytes[..end], false),
            "prefix {end}"
        );
    }
}

#[test]
fn file_scan_matches_slice_scan_malformed_wire() {
    for bytes in [
        vec![0],
        vec![0x80],
        vec![0x80; 11],
        vec![0x0b],
        vec![0x0f],
        field(FIELD_SBOOK_AUTHOR, &[0xff]),
        field(FIELD_SBOOK_STATE, &[0]),
        field(FIELD_SBOOK_STATE, &[0x0b]),
        field(FIELD_SBOOK_STATE, &[8, 0x80]),
        field(FIELD_SBOOK_STATE, &[0x12, 5, 0]),
    ] {
        assert_eq!(scan_result(&bytes, true), scan_result(&bytes, false), "{bytes:?}");
    }
}

#[test]
fn file_scan_seeks_over_state_payload_after_first_id() {
    struct SmallReads(Cursor<Vec<u8>>);
    impl Read for SmallReads {
        fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
            assert!(bytes.len() <= 1, "state payload must be skipped");
            self.0.read(bytes)
        }
    }
    impl Seek for SmallReads {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.0.seek(position)
        }
    }
    let mut payload = vec![8, 37];
    payload.resize(1024 * 1024, 0xff);
    let bytes = field(FIELD_SBOOK_STATE, &payload);
    let scan = scan_file(&mut SmallReads(Cursor::new(bytes))).expect("scan large state");
    assert_eq!(scan.states[0].state_id, Some(37));
    assert_eq!(scan.states[0].payload_bytes, payload.len());
}

#[test]
fn file_fields_copy_original_wire_spans() {
    let bytes = [field(100, b"opaque"), vec![8, 0x81, 0x00], vec![0x15, 1, 2, 3, 4]].concat();
    let mut input = Cursor::new(&bytes);
    let mut output = Vec::new();
    while let Some(field) = next_field(&mut input, bytes.len() as u64).expect("field") {
        copy_span(&mut input, &mut output, field.start, field.end).expect("copy field");
    }
    assert_eq!(output, bytes);
}

#[test]
fn buffered_file_seeks_and_reads_match_cursor() {
    let bytes: Vec<u8> = (0..20_000).map(|index| (index % 251) as u8).collect();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    let path = std::env::temp_dir()
        .join(format!("rsshogi_sbk_buffered_{}_{nonce}.sbk", std::process::id()));
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("temporary file");
    file.write_all(&bytes).expect("write fixture");
    file.seek(SeekFrom::Start(3)).expect("initial position");
    let mut input = BufferedFile::new(file).expect("buffered file");
    let mut expected = Cursor::new(&bytes);
    expected.set_position(3);
    assert_eq!(input.len(), bytes.len() as u64);
    assert_eq!(input.stream_position().unwrap(), 3);
    let mut byte = [0];
    input.read_exact(&mut byte).unwrap();
    let buffered = input.reader.buffer().len();
    assert!(buffered > 0);
    input.seek(SeekFrom::Start(3)).unwrap();
    assert_eq!(input.reader.buffer().len(), buffered + 1);

    for seek in [
        SeekFrom::Current(0),
        SeekFrom::Start(10),
        SeekFrom::Current(-5),
        SeekFrom::End(-20),
        SeekFrom::Start(8190),
        SeekFrom::Current(-8100),
        SeekFrom::End(10),
        SeekFrom::Current(-100),
        SeekFrom::Start(0),
    ] {
        assert_eq!(input.seek(seek).unwrap(), expected.seek(seek).unwrap());
        let mut actual_bytes = [0; 32];
        let mut expected_bytes = [0; 32];
        assert_eq!(
            input.read(&mut actual_bytes).unwrap(),
            expected.read(&mut expected_bytes).unwrap()
        );
        assert_eq!(actual_bytes, expected_bytes);
        assert_eq!(input.stream_position().unwrap(), expected.position());
        assert_eq!(input.stream_position().unwrap(), expected.position());
    }

    for seek in [SeekFrom::Start(0), SeekFrom::Start(10)] {
        input.seek(seek).unwrap();
        let position = input.stream_position().unwrap();
        for invalid in [SeekFrom::Current(-11), SeekFrom::End(-20_001)] {
            assert_eq!(input.seek(invalid).unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(input.stream_position().unwrap(), position);
        }
        let mut actual = [0; 10];
        input.read_exact(&mut actual).expect("read after failed seek");
        assert_eq!(actual, bytes[position as usize..position as usize + 10]);
    }
    drop(input);
    std::fs::remove_file(path).expect("remove fixture");
}
