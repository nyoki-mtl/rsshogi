//! 実定跡の保存配置を比較するための実験用抽出器。
//!
//! 出力は正式な Native Book 形式ではない。回数を除く値と表示順を保持する。

use rsshogi::board::{PackedSfen, Position};
use rsshogi::book::{YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookOpenOptions};
use rsshogi::records::formats::sbinpack::ordered_legal_moves;
use rsshogi::types::{Move, Square};
use serde_json::json;
use std::collections::BTreeMap;
use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn create(path: impl AsRef<Path>) -> Result<BufWriter<File>> {
    Ok(BufWriter::with_capacity(
        1024 * 1024,
        OpenOptions::new().write(true).create_new(true).open(path)?,
    ))
}

fn extract(source: &str, output: &Path, limit: u64) -> Result<()> {
    fs::create_dir_all(output)?;
    let started = Instant::now();
    let book = YaneuraOuBook::open_with_options(
        source,
        YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
    )?;
    let mut payload = create(output.join("candidates16.bin"))?;
    let mut index: Vec<[u8; 40]> = Vec::new();
    let mut offset = 0u64;
    let mut counts = BTreeMap::<usize, u64>::new();
    let mut depths = BTreeMap::<u32, u64>::new();
    let mut score_min = i32::MAX;
    let mut score_max = i32::MIN;
    let mut large_depths = 0u64;
    let mut absent_scores = 0u64;
    let mut absent_depths = 0u64;
    let mut original_counts = 0u64;
    let mut nonempty_comments = 0u64;
    let mut ponders = 0u64;
    for entry in book.iter_entries()? {
        let entry = entry?;
        let pos = Position::from_sfen(entry.sfen())?;
        let packed = pos.try_to_packed_sfen()?;
        let mut row = [0u8; 40];
        row[..32].copy_from_slice(packed.as_bytes());
        row[32..].copy_from_slice(&offset.to_le_bytes());
        index.push(row);
        let k = u16::try_from(entry.moves().len())?;
        payload.write_all(&k.to_le_bytes())?;
        offset += 2;
        *counts.entry(usize::from(k)).or_default() += 1;
        nonempty_comments += u64::from(!entry.comment().is_empty());
        for (order, candidate) in entry.moves().iter().enumerate() {
            let mut bytes = [0u8; 16];
            bytes[..2].copy_from_slice(&candidate.mv().raw().to_le_bytes());
            bytes[2] = u8::from(candidate.score().is_some());
            bytes[4..8].copy_from_slice(&candidate.score().unwrap_or(0).to_le_bytes());
            bytes[8..12].copy_from_slice(&candidate.depth().unwrap_or(0).to_le_bytes());
            bytes[12..14].copy_from_slice(&u16::from(candidate.depth().is_some()).to_le_bytes());
            bytes[14..].copy_from_slice(&u16::try_from(order)?.to_le_bytes());
            payload.write_all(&bytes)?;
            offset += 16;
            if let Some(score) = candidate.score() {
                score_min = score_min.min(score);
                score_max = score_max.max(score);
            } else {
                absent_scores += 1;
            }
            if let Some(depth) = candidate.depth() {
                *depths.entry(depth).or_default() += 1;
                large_depths += u64::from(depth > u32::from(u16::MAX));
            } else {
                absent_depths += 1;
            }
            original_counts += u64::from(candidate.count().is_some());
            nonempty_comments += u64::from(!candidate.comment().is_empty());
            ponders += u64::from(candidate.ponder().is_normal());
        }
        if index.len().is_multiple_of(1_000_000) {
            eprintln!(
                "extracted {} positions in {:.1}s",
                index.len(),
                started.elapsed().as_secs_f64()
            );
        }
        if limit > 0 && index.len() as u64 >= limit {
            break;
        }
    }
    payload.flush()?;
    let n = index.len();
    eprintln!("sorting {n} keys");
    index.sort_unstable();
    let duplicates = index.windows(2).filter(|pair| pair[0][..32] == pair[1][..32]).count();
    if duplicates != 0 {
        return Err(format!("duplicate canonical keys: {duplicates}").into());
    }
    let mut keys = create(output.join("sorted_index40.bin"))?;
    for row in &index {
        keys.write_all(row)?;
    }
    keys.flush()?;
    let m: u64 = counts.iter().map(|(k, n)| *k as u64 * n).sum();
    let report = json!({
        "source":source, "source_bytes":fs::metadata(source)?.len(),
        "limit":limit, "positions":n, "candidates":m,
        "candidate_histogram":counts, "depth_histogram":depths,
        "score_min":score_min, "score_max":score_max,
        "depth_over_u16":large_depths, "absent_scores":absent_scores,
        "absent_depths":absent_depths, "original_count_present":original_counts,
        "nonempty_comments":nonempty_comments, "normal_ponders":ponders,
        "canonical_duplicate_keys":duplicates, "seconds":started.elapsed().as_secs_f64(),
        "native24_bytes":136 + n as u64 * 40 + m * 24,
        "no_count16_bytes":136 + n as u64 * 40 + m * 16,
        "no_count12_bytes":136 + n as u64 * 40 + m * 12,
    });
    let mut report_file = create(output.join("extract.json"))?;
    serde_json::to_writer_pretty(&mut report_file, &report)?;
    report_file.flush()?;
    println!("{report}");
    Ok(())
}

fn extract_ybb(source: &str, output: &Path) -> Result<()> {
    fs::create_dir_all(output)?;
    let started = Instant::now();
    let bytes = fs::read(source)?;
    if bytes.len() < 32 || &bytes[..16] != b"YANE-BINBOOK-V1\0" {
        return Err("invalid YBB header".into());
    }
    let n = usize::try_from(u64::from_le_bytes(bytes[16..24].try_into()?))?;
    let flags = u64::from_le_bytes(bytes[24..32].try_into()?);
    if flags & !1 != 0 {
        return Err("unknown YBB flags".into());
    }
    let has_depth = flags & 1 != 0;
    let width = if has_depth { 6 } else { 4 };
    let move_area =
        32usize.checked_add(n.checked_mul(44).ok_or("index overflow")?).ok_or("index overflow")?;
    if move_area > bytes.len() {
        return Err("truncated YBB index".into());
    }
    let mut payload = create(output.join("candidates16.bin"))?;
    let mut index = create(output.join("sorted_index40.bin"))?;
    let mut previous: Option<[u8; 32]> = None;
    let mut counts = BTreeMap::<usize, u64>::new();
    let mut depths = BTreeMap::<u32, u64>::new();
    let mut offset = 0u64;
    let mut m = 0u64;
    let mut score_min = i32::MAX;
    let mut score_max = i32::MIN;
    let mut ply_min = u16::MAX;
    let mut ply_max = 0u16;
    for i in 0..n {
        let start = 32 + i * 44;
        let key: [u8; 32] = bytes[start..start + 32].try_into()?;
        if previous.is_some_and(|p| p >= key) {
            return Err(format!("duplicate or unsorted key at {i}").into());
        }
        previous = Some(key);
        let mut pos = Position::empty();
        pos.set_packed_sfen(&PackedSfen::from_bytes(key), false, 1)?;
        if pos.try_to_packed_sfen()?.as_bytes() != &key {
            return Err(format!("noncanonical key at {i}").into());
        }
        let source_offset =
            usize::try_from(u64::from_le_bytes(bytes[start + 32..start + 40].try_into()?))?;
        let ply = u16::from_le_bytes(bytes[start + 40..start + 42].try_into()?);
        ply_min = ply_min.min(ply);
        ply_max = ply_max.max(ply);
        let k = u16::from_le_bytes(bytes[start + 42..start + 44].try_into()?);
        let begin = move_area.checked_add(source_offset).ok_or("move offset overflow")?;
        let end = begin.checked_add(usize::from(k) * width).ok_or("move extent overflow")?;
        if end > bytes.len() {
            return Err(format!("truncated moves at {i}").into());
        }
        index.write_all(&key)?;
        index.write_all(&offset.to_le_bytes())?;
        payload.write_all(&k.to_le_bytes())?;
        offset += 2;
        *counts.entry(usize::from(k)).or_default() += 1;
        let mut seen = Vec::with_capacity(usize::from(k));
        for order in 0..usize::from(k) {
            let at = begin + order * width;
            let raw = u16::from_le_bytes(bytes[at..at + 2].try_into()?);
            let mv = Move::from_raw(raw);
            if !mv.is_normal()
                || mv.to_sq().raw() < 0
                || mv.to_sq().raw() >= Square::COUNT as i8
                || (mv.is_drop() && (mv.is_promotion() || !matches!((raw >> 7) & 127, 1..=7)))
                || (!mv.is_drop()
                    && (mv.from_sq().raw() < 0
                        || mv.from_sq().raw() >= Square::COUNT as i8
                        || mv.from_sq() == mv.to_sq()))
            {
                return Err(format!("invalid move at {i}:{order}").into());
            }
            if seen.contains(&raw) {
                return Err(format!("duplicate move at {i}:{order}").into());
            }
            seen.push(raw);
            let score = i32::from(i16::from_le_bytes(bytes[at + 2..at + 4].try_into()?));
            let depth = if has_depth {
                u32::from(u16::from_le_bytes(bytes[at + 4..at + 6].try_into()?))
            } else {
                0
            };
            score_min = score_min.min(score);
            score_max = score_max.max(score);
            if has_depth {
                *depths.entry(depth).or_default() += 1;
            }
            let mut candidate = [0u8; 16];
            candidate[..2].copy_from_slice(&raw.to_le_bytes());
            candidate[2] = 1;
            candidate[4..8].copy_from_slice(&score.to_le_bytes());
            candidate[8..12].copy_from_slice(&depth.to_le_bytes());
            candidate[12..14].copy_from_slice(&u16::from(has_depth).to_le_bytes());
            candidate[14..].copy_from_slice(&(order as u16).to_le_bytes());
            payload.write_all(&candidate)?;
            offset += 16;
            m += 1;
        }
        if (i + 1).is_multiple_of(1_000_000) {
            eprintln!("extracted {} positions in {:.1}s", i + 1, started.elapsed().as_secs_f64());
        }
    }
    payload.flush()?;
    index.flush()?;
    let report = json!({"source":source,"source_format":"YANE-BINBOOK-V1","source_bytes":bytes.len(),
        "positions":n,"candidates":m,"has_depth":has_depth,"candidate_histogram":counts,
        "depth_histogram":depths,"score_min":score_min,"score_max":score_max,"ply_min":ply_min,"ply_max":ply_max,
        "canonical_duplicate_keys":0,"duplicate_candidate_moves":0,"noncanonical_keys":0,
        "native24_bytes":136+n as u64*40+m*24,"no_count16_bytes":136+n as u64*40+m*16,
        "seconds":started.elapsed().as_secs_f64()});
    let mut report_file = create(output.join("extract.json"))?;
    serde_json::to_writer_pretty(&mut report_file, &report)?;
    report_file.flush()?;
    println!("{report}");
    Ok(())
}

fn legal_sample(input: &Path, output: &Path) -> Result<()> {
    let mut reader = File::open(input)?;
    let mut writer = create(output)?;
    let mut n = 0u64;
    let mut m = 0u64;
    let mut one_byte = 0u64;
    let mut max_index = 0usize;
    let mut legal_total = 0usize;
    let mut invalid = 0u64;
    let mut nanos = Vec::new();
    let started = Instant::now();
    loop {
        let mut key = [0u8; 32];
        if reader.read(&mut key[..1])? == 0 {
            break;
        }
        reader.read_exact(&mut key[1..])?;
        let mut count = [0u8; 2];
        reader.read_exact(&mut count)?;
        writer.write_all(&key)?;
        writer.write_all(&count)?;
        let k = u16::from_le_bytes(count);
        let mut pos = Position::empty();
        pos.set_packed_sfen(&PackedSfen::from_bytes(key), false, 1)?;
        let t = Instant::now();
        let moves = ordered_legal_moves(&pos);
        nanos.push(t.elapsed().as_nanos() as u64);
        legal_total += moves.len();
        for _ in 0..k {
            let mut candidate = [0u8; 16];
            reader.read_exact(&mut candidate)?;
            let raw = u16::from_le_bytes(candidate[..2].try_into()?);
            if let Some(index) = moves.iter().position(|mv| mv.to_move().raw() == raw) {
                candidate[..2].copy_from_slice(&u16::try_from(index)?.to_le_bytes());
                max_index = max_index.max(index);
                one_byte += u64::from(index < 128);
            } else {
                invalid += 1;
            }
            writer.write_all(&candidate)?;
            m += 1;
        }
        n += 1;
    }
    writer.flush()?;
    nanos.sort_unstable();
    if n == 0 || invalid != 0 {
        return Err(format!("empty or illegal sample: n={n}, invalid={invalid}").into());
    }
    println!(
        "{}",
        json!({"positions":n,"candidates":m,"one_byte_indices":one_byte,
        "max_index":max_index,"legal_moves_mean":legal_total as f64 / n as f64,
        "ordering_p50_us":nanos[nanos.len()/2] as f64 / 1000.0,
        "ordering_p95_us":nanos[(nanos.len()*95/100).min(nanos.len()-1)] as f64 / 1000.0,
        "invalid_candidates":invalid,"seconds":started.elapsed().as_secs_f64()})
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("extract-ybb") if args.len() == 4 => extract_ybb(&args[2], Path::new(&args[3])),
        Some("extract") if args.len() == 4 || args.len() == 5 => extract(
            &args[2], Path::new(&args[3]), args.get(4).map(|s| s.parse()).transpose()?.unwrap_or(0),
        ),
        Some("legal") if args.len() == 4 => legal_sample(Path::new(&args[2]), Path::new(&args[3])),
        _ => Err("usage: native_book_layout_probe extract <db> <directory> [limit] | legal <sample> <output>".into()),
    }
}
