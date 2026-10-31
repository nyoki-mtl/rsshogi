use super::*;

fn independent_bytes(entries: &[YbbEntry], depth: bool) -> Vec<u8> {
    let mut bytes = Vec::from(*MAGIC);
    bytes.extend_from_slice(&(entries.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&u64::from(depth).to_le_bytes());
    let mut moves = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(entry.packed_sfen.as_bytes());
        bytes.extend_from_slice(&(moves.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&entry.ply.to_le_bytes());
        bytes.extend_from_slice(&(entry.moves.len() as u16).to_le_bytes());
        for mv in &entry.moves {
            moves.extend_from_slice(&mv.raw_move.to_le_bytes());
            moves.extend_from_slice(&mv.eval.to_le_bytes());
            if let Some(depth) = mv.depth {
                moves.extend_from_slice(&depth.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&moves);
    bytes
}

#[test]
fn writer_blocks_match_independent_bytes_with_bounded_capacity() {
    let key = hirate_position().to_packed_sfen();
    for depth in [false, true] {
        let mut entries = Vec::new();
        for n in 0..4500 {
            let count = if n == 1800 {
                u16::MAX as usize
            } else if n % 3 == 0 {
                0
            } else {
                17
            };
            entries.push(
                YbbEntry::new(key, n, vec![candidate("2g2f", i16::MIN, depth.then_some(0)); count])
                    .unwrap(),
            );
        }
        let mut writer = YbbWriter::new(
            Cursor::new(Vec::new()),
            entries.len() as u64,
            YbbWriteOptions::new().with_depth(depth),
        )
        .unwrap();
        for entry in &entries {
            writer.write_entry(entry).unwrap();
            assert!(writer.index.capacity() <= WRITE_BUFFER_BYTES / INDEX_BYTES);
            assert!(writer.moves.capacity() <= u16::MAX as usize * 6);
            assert!(writer.index.len() <= WRITE_BUFFER_BYTES / INDEX_BYTES);
            assert!(writer.moves.len() < WRITE_BUFFER_BYTES);
        }
        let output = writer.finish().unwrap();
        assert_eq!(output.position(), output.get_ref().len() as u64);
        assert_eq!(output.into_inner(), independent_bytes(&entries, depth));
    }
}

#[test]
fn zero_move_rows_flush_index_blocks_and_late_io_errors_poison_writer() {
    let entry = YbbEntry::new(hirate_position().to_packed_sfen(), 0, vec![]).unwrap();
    let mut writer = YbbWriter::new(FaultOutput::default(), 5000, YbbWriteOptions::new()).unwrap();
    for _ in 0..WRITE_BUFFER_BYTES / INDEX_BYTES {
        writer.write_entry(&entry).unwrap();
    }
    assert_eq!(writer.flushed_row_count, 0);
    writer.output.write_budget = Some(1);
    assert!(matches!(writer.write_entry(&entry), Err(BookError::Io(_))));
    writer.output.write_budget = None;
    assert!(writer.write_entry(&entry).is_err());
    assert!(writer.finish().is_err());
    let entries = vec![entry; 5000];
    assert_eq!(encode(&entries, false), independent_bytes(&entries, false));
}

#[test]
fn short_and_interrupted_writes_are_retried_across_blocks() {
    #[derive(Default)]
    struct ShortOutput {
        cursor: Cursor<Vec<u8>>,
        interrupted: bool,
    }
    impl Write for ShortOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if !self.interrupted {
                self.interrupted = true;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            self.cursor.write(&bytes[..bytes.len().min(7)])
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Seek for ShortOutput {
        fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
            self.cursor.seek(offset)
        }
    }
    let entry = YbbEntry::new(
        hirate_position().to_packed_sfen(),
        0,
        vec![candidate("7g7f", i16::MAX, Some(u16::MAX))],
    )
    .unwrap();
    let entries = vec![entry; 2000];
    let mut writer = YbbWriter::new(
        ShortOutput::default(),
        entries.len() as u64,
        YbbWriteOptions::new().with_depth(true),
    )
    .unwrap();
    for entry in &entries {
        writer.write_entry(entry).unwrap();
    }
    assert_eq!(writer.finish().unwrap().cursor.into_inner(), independent_bytes(&entries, true));
}
