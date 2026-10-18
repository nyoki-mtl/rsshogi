//! YBB 原本と本番 Native Book の全件一致を検証する。

use std::error::Error;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use rsshogi::board::{self, PackedSfen, Position};
use rsshogi::book::{
    BookControl, NativeBook, NativeBookBuilder, NativeBookEntry, NativeBookMetadata,
    NativeBookPosition, NativeBound, NativeCandidate, NativeScore,
};
use rsshogi::types::{Move, Square};
use serde_json::json;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct Source {
    bytes: Vec<u8>,
    positions: usize,
    move_area: usize,
    has_depth: bool,
}

impl Source {
    fn open(path: &Path) -> Result<Self> {
        let bytes = fs::read(path)?;
        if bytes.len() < 32 || &bytes[..16] != b"YANE-BINBOOK-V1\0" {
            return Err("invalid YBB header".into());
        }
        let positions = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into()?))?;
        let flags = u64::from_le_bytes(bytes[24..32].try_into()?);
        if flags & !1 != 0 {
            return Err("unknown YBB flags".into());
        }
        let move_area = positions
            .checked_mul(44)
            .and_then(|size| size.checked_add(32))
            .ok_or("YBB index overflow")?;
        if move_area > bytes.len() {
            return Err("truncated YBB index".into());
        }
        Ok(Self { bytes, positions, move_area, has_depth: flags & 1 != 0 })
    }

    fn row(&self, index: usize) -> Result<Row<'_>> {
        if index >= self.positions {
            return Err("YBB position index out of range".into());
        }
        let start = 32 + index * 44;
        let key = self.bytes[start..start + 32].try_into()?;
        let offset =
            usize::try_from(u64::from_le_bytes(self.bytes[start + 32..start + 40].try_into()?))?;
        let ply = u16::from_le_bytes(self.bytes[start + 40..start + 42].try_into()?);
        let count = u16::from_le_bytes(self.bytes[start + 42..start + 44].try_into()?);
        let width = if self.has_depth { 6 } else { 4 };
        let begin = self.move_area.checked_add(offset).ok_or("YBB move offset overflow")?;
        let end =
            begin.checked_add(usize::from(count) * width).ok_or("YBB move extent overflow")?;
        let moves = self.bytes.get(begin..end).ok_or("truncated YBB moves")?;
        Ok(Row { key, ply, moves, width })
    }
}

struct Row<'a> {
    key: [u8; 32],
    ply: u16,
    moves: &'a [u8],
    width: usize,
}

impl Row<'_> {
    fn raw_candidates(&self) -> Vec<(u16, i32, Option<u32>, u16)> {
        self.moves
            .chunks_exact(self.width)
            .enumerate()
            .map(|(order, bytes)| {
                let raw = u16::from_le_bytes(bytes[..2].try_into().expect("move bytes"));
                let score =
                    i32::from(i16::from_le_bytes(bytes[2..4].try_into().expect("score bytes")));
                let depth = (self.width == 6).then(|| {
                    u32::from(u16::from_le_bytes(bytes[4..6].try_into().expect("depth bytes")))
                });
                (raw, score, depth, order as u16)
            })
            .collect()
    }
}

fn run(command: &str, source_path: &Path, book_path: &Path, limit: usize) -> Result<()> {
    board::init();
    let started = Instant::now();
    let source = Source::open(source_path)?;
    let positions = if limit == 0 { source.positions } else { limit.min(source.positions) };
    let mut builder = if command == "build" {
        Some(NativeBookBuilder::new(book_path, NativeBookMetadata::default())?)
    } else {
        None
    };
    let book = if command == "verify" {
        let book = NativeBook::open(book_path)?;
        if book.metadata()?.depth_kind != 0 {
            return Err("YBB depth meaning is unconfirmed: expected depth_kind=0".into());
        }
        book.validate_full_with_control(|progress| {
            if progress.phase == "positions" && progress.completed.is_multiple_of(1_000_000) {
                eprintln!(
                    "validated {} positions in {:.1}s",
                    progress.completed,
                    started.elapsed().as_secs_f64()
                );
            }
            BookControl::Continue
        })?;
        Some(book)
    } else {
        None
    };
    let validation_seconds = started.elapsed().as_secs_f64();
    let mut entries = book.as_ref().map(NativeBook::entries);
    let mut previous = None;
    let mut candidates = 0u64;
    let mut ply_min = u16::MAX;
    let mut ply_max = 0;
    for index in 0..positions {
        let row = source.row(index)?;
        if previous.is_some_and(|key| key >= row.key) {
            return Err(format!("duplicate or unsorted YBB key at {index}").into());
        }
        previous = Some(row.key);
        ply_min = ply_min.min(row.ply);
        ply_max = ply_max.max(row.ply);
        let mut raw_candidates = row.raw_candidates();
        candidates += raw_candidates.len() as u64;
        if let Some(builder) = &mut builder {
            let mut position = Position::empty();
            position.set_packed_sfen(&PackedSfen::from_bytes(row.key), false, 1)?;
            let mut native_candidates = Vec::with_capacity(raw_candidates.len());
            for (raw, value, depth, display_order) in raw_candidates {
                let mv = Move::from_raw(raw);
                if !mv.is_normal()
                    || !(0..Square::COUNT as i8).contains(&mv.to_sq().raw())
                    || (mv.is_drop() && (mv.is_promotion() || !matches!((raw >> 7) & 127, 1..=7)))
                    || (!mv.is_drop()
                        && (!(0..Square::COUNT as i8).contains(&mv.from_sq().raw())
                            || mv.from_sq() == mv.to_sq()))
                {
                    return Err(format!("invalid YBB move at {index}:{display_order}").into());
                }
                native_candidates.push(NativeCandidate {
                    mv,
                    score: NativeScore::Cp { value, bound: NativeBound::Exact },
                    depth,
                    display_order,
                });
            }
            let entry = NativeBookEntry::from_position(&position, native_candidates)?;
            if entry.key != row.key {
                return Err(format!("noncanonical YBB key at {index}").into());
            }
            builder.push(NativeBookPosition::new(entry))?;
        } else if let Some(entries) = &mut entries {
            let entry = entries.next().ok_or("Native Book has fewer positions than YBB")??;
            if entry.key != row.key {
                return Err(format!("key mismatch at {index}").into());
            }
            raw_candidates.sort_unstable_by_key(|candidate| candidate.0);
            if entry.candidates.len() != raw_candidates.len() {
                return Err(format!("candidate count mismatch at {index}").into());
            }
            for (candidate, &(raw, value, depth, display_order)) in
                entry.candidates.iter().zip(&raw_candidates)
            {
                if candidate.mv.raw() != raw
                    || candidate.score != (NativeScore::Cp { value, bound: NativeBound::Exact })
                    || candidate.depth != depth
                    || candidate.display_order != display_order
                {
                    return Err(format!("candidate mismatch at {index}, source move {raw}").into());
                }
            }
        }
        if (index + 1).is_multiple_of(1_000_000) {
            eprintln!(
                "{command}: {} positions, {candidates} candidates in {:.1}s",
                index + 1,
                started.elapsed().as_secs_f64()
            );
        }
    }
    if let Some(entries) = &mut entries
        && entries.next().transpose()?.is_some()
    {
        return Err("Native Book has extra positions after YBB comparison".into());
    }
    if let Some(builder) = builder {
        builder.finish()?;
    }
    println!(
        "{}",
        json!({
            "command": command, "source": source_path, "book": book_path,
            "source_bytes": source.bytes.len(), "source_positions": source.positions,
            "positions": positions, "candidates": candidates, "limit": limit,
            "source_has_depth": source.has_depth, "depth_kind": 0,
            "source_original_ply_discarded": true,
            "source_ply_min": (positions != 0).then_some(ply_min),
            "source_ply_max": (positions != 0).then_some(ply_max),
            "book_bytes": fs::metadata(book_path)?.len(),
            "full_validation_passed": command == "verify",
            "all_fields_match": command == "verify", "extra_entries": 0,
            "initialization_and_validation_seconds": validation_seconds,
            "elapsed_seconds": started.elapsed().as_secs_f64()
        })
    );
    Ok(())
}

fn latency_report(times: &mut [u64]) -> serde_json::Value {
    times.sort_unstable();
    let percentile = |percent: usize| {
        times
            .get(times.len().saturating_mul(percent).div_ceil(100).saturating_sub(1))
            .map(|&ns| ns as f64 / 1000.0)
    };
    json!({
        "count": times.len(), "p50_us": percentile(50), "p95_us": percentile(95),
        "p99_us": percentile(99), "max_us": times.last().map(|&ns| ns as f64 / 1000.0)
    })
}

fn measure(source_path: &Path, book_path: &Path, query_count: usize) -> Result<()> {
    if query_count == 0 {
        return Err("measurement query count must be positive".into());
    }
    let source = Source::open(source_path)?;
    if source.positions == 0 {
        return Err("measurement source must contain positions".into());
    }
    let samples = source.positions.min(10_000);
    let mut keys = Vec::with_capacity(samples * 2);
    for sample in 0..samples {
        let index = if samples == 1 { 0 } else { sample * (source.positions - 1) / (samples - 1) };
        let key = source.row(index)?.key;
        keys.push(key);
        let mut probe = key;
        probe[0] ^= 0x80;
        keys.push(probe);
    }
    drop(source);
    let mut header = [0; 128];
    fs::File::open(book_path)?.read_exact(&mut header)?;
    let index_bytes = u64::from_le_bytes(header[56..64].try_into()?)
        .checked_sub(u64::from_le_bytes(header[48..56].try_into()?))
        .ok_or("invalid native block index extent")?;
    let warmup = NativeBook::open(book_path)?;
    for &key in &keys {
        warmup.get_key(key)?;
    }
    drop(warmup);
    let open_started = Instant::now();
    let book = NativeBook::open(book_path)?;
    let open_ms = open_started.elapsed().as_secs_f64() * 1000.0;
    let before = book.io_stats();
    let mut hits = Vec::new();
    let mut misses = Vec::new();
    let mut source_key_hits = 0u64;
    let mut mutated_key_hits = 0u64;
    let lookup_started = Instant::now();
    for query in 0..query_count {
        let key = keys[query % keys.len()];
        let started = Instant::now();
        let entry = book.get_key(key)?;
        let ns = u64::try_from(started.elapsed().as_nanos())?;
        if entry.is_some() {
            hits.push(ns);
            if query.is_multiple_of(2) {
                source_key_hits += 1;
            } else {
                mutated_key_hits += 1;
            }
        } else {
            misses.push(ns);
        }
    }
    let elapsed_seconds = lookup_started.elapsed().as_secs_f64();
    let after = book.io_stats();
    println!(
        "{}",
        json!({
            "command": "measure", "pid": std::process::id(), "source": source_path,
            "book": book_path, "sample_source_keys": samples, "queries": query_count,
            "sample_rule": "uniform source row indices including endpoints; alternating source key and first-byte XOR 0x80 probe; repeated deterministically",
            "classification": "actual get_key result; mutated probes may be hits",
            "source_key_hits": source_key_hits, "mutated_key_hits": mutated_key_hits,
            "open_ms": open_ms, "block_index_bytes": index_bytes,
            "open_read_calls": before.read_calls, "open_read_bytes": before.read_bytes,
            "hit": latency_report(&mut hits), "miss": latency_report(&mut misses),
            "elapsed_lookup_seconds": elapsed_seconds,
            "lookup_read_calls": after.read_calls - before.read_calls,
            "lookup_read_bytes": after.read_bytes - before.read_bytes,
            "lookup_seek_calls": after.seek_calls - before.seek_calls,
            "lookup_read_syscalls": after.read_syscalls - before.read_syscalls,
            "comment_api_requests": 0, "lookup_api": "NativeBook::get_key",
            "qualification": "warm OS cache after one full sample pass; source loading and query preparation excluded; application requested I/O counters are not physical disk I/O"
        })
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if !(args.len() == 4 || args.len() == 5)
        || !matches!(args[1].as_str(), "build" | "verify" | "measure")
    {
        return Err(
            "usage: native_book_ybb_verify <build|verify|measure> <source.ybb> <output.book> [limit_or_queries]"
                .into(),
        );
    }
    let limit = args
        .get(4)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(if args[1] == "measure" { 100_000 } else { 0 });
    if args[1] == "measure" {
        return measure(Path::new(&args[2]), Path::new(&args[3]), limit);
    }
    run(&args[1], Path::new(&args[2]), Path::new(&args[3]), limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_candidates_preserve_signed_scores_depth_presence_and_source_order() {
        let bytes = [0x34, 0x12, 0x00, 0x80, 0x00, 0x00, 0x78, 0x56, 0xff, 0x7f, 0xff, 0xff];
        let row = Row { key: [0; 32], ply: 123, moves: &bytes, width: 6 };
        assert_eq!(
            row.raw_candidates(),
            vec![(0x1234, -32768, Some(0), 0), (0x5678, 32767, Some(65535), 1)]
        );
        let row = Row { key: [0; 32], ply: 0, moves: &bytes[..4], width: 4 };
        assert_eq!(row.raw_candidates(), vec![(0x1234, -32768, None, 0)]);
    }

    #[test]
    fn source_row_rejects_truncated_moves_and_out_of_range_offsets() {
        let mut source =
            Source { bytes: vec![0; 76], positions: 1, move_area: 76, has_depth: true };
        source.bytes[74..76].copy_from_slice(&1u16.to_le_bytes());
        assert!(source.row(0).is_err());
        source.bytes[64..72].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(source.row(0).is_err());
        assert!(source.row(1).is_err());
    }
}
