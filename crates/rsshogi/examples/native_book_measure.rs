//! Native Book の決定的 fixture 生成と独立プロセスでの warm 計測。
//!
//! fixture は到達手順や棋力を表さず、合法性と I/O 量の試験だけに使う。
//! 両側の通常初期配置の最下段を維持し、飛角を各側の持ち駒へ移す。
//! 白歩を各筋 b/c/d、黒歩を各筋 f/g/h に置く 18 桁の三進数で局面を識別する。
//! seed から作る 3 と互いに素な乗数と加算によって三進数空間を全単射で並べ替える。
//! 手番は黒固定、駒の総数は平手と同じで、二歩と行き所のない駒は発生しない。
//! 生成順の入力ハッシュは明示した FNV-1a 64-bit であり、暗号学的認証ではない。
//!
//! ```text
//! native_book_measure generate <path> <count> <seed> [--comments]
//! native_book_measure measure <path> <repeats> <output.json> [--queries <even-count>]
//! native_book_measure edits <source> <destination> <count> <output.json> [--repeat-position]
//! native_book_measure validate <path> <output.json>
//! native_book_measure compact <source> <destination> <output.json>
//! native_book_measure sorted <source> <destination> <output.json>
//! ```
//!
//! 各反復は別プロセスで同じ検索列を warm up してから計測する。
//! OS cache を排除しないため cold 計測とは呼ばない。
//! メモリと物理 I/O は外部のプロセス計測と別途対応付ける。

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use rsshogi::board::{self, LegalAll, MoveList, Position, generate_moves};
use rsshogi::book::{
    NativeBook, NativeBookBuilder, NativeBookEditor, NativeBookEntry, NativeBookMetadata,
    NativeBookPosition, NativeBound, NativeCandidate, NativeScore,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const RULE: &str = "native-v4-ternary-pawns-v1";
const POSITION_SPACE: u64 = 387_420_489;
const MAX_POSITIONS: u64 = 100_000_000;
const DEFAULT_QUERIES: usize = 100_000;
const LOOKUP_SEED: u64 = 0x626f_6f6b_7633_7172;
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[cfg(windows)]
mod memory {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Counters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_nonpaged_pool: usize,
        quota_nonpaged_pool: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
        private_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut Counters,
            bytes: u32,
        ) -> i32;
    }

    pub fn snapshot() -> std::io::Result<serde_json::Value> {
        let mut counters = Counters { cb: size_of::<Counters>() as u32, ..Counters::default() };
        // SAFETY: 現在のプロセスの疑似 handle と、初期化済みの
        // PROCESS_MEMORY_COUNTERS_EX と同じ C 配置の書込可能な領域を渡す。
        let success =
            unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) };
        if success == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(serde_json::json!({
            "private_bytes": counters.private_usage,
            "peak_private_commit_bytes": counters.peak_pagefile_usage,
            "working_set_bytes": counters.working_set,
            "peak_working_set_bytes": counters.peak_working_set,
            "method": "K32GetProcessMemoryInfo PROCESS_MEMORY_COUNTERS_EX; peak commit is a conservative process-lifetime upper bound"
        }))
    }
}

fn memory_snapshot() -> Result<Value> {
    #[cfg(windows)]
    {
        Ok(memory::snapshot()?)
    }
    #[cfg(not(windows))]
    {
        Ok(json!({"method": "not measured on this platform"}))
    }
}

fn memory_difference(baseline: &Value, after: &Value) -> Value {
    let additional = baseline["private_bytes"]
        .as_u64()
        .zip(after["peak_private_commit_bytes"].as_u64())
        .map(|(baseline, peak)| peak.saturating_sub(baseline));
    json!({"baseline": baseline, "after": after, "additional_peak_private_upper_bound_bytes": additional})
}

fn write_report(path: &Path, report: &Value) -> Result<()> {
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, report)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    writer.get_ref().sync_all()?;
    println!("{}", serde_json::to_string_pretty(report)?);
    Ok(())
}

fn file_layout(path: &Path) -> Result<Value> {
    let mut header = [0; 128];
    let mut file = BufReader::new(std::fs::File::open(path)?);
    file.read_exact(&mut header)?;
    if &header[..8] != b"RSSHGBK4" {
        return Err("unsupported measurement book layout".into());
    }
    let base_end = u64::from_le_bytes(header[16..24].try_into()?);
    let positions = u64::from_le_bytes(header[24..32].try_into()?);
    let index_offset = u64::from_le_bytes(header[48..56].try_into()?);
    let blocks_offset = u64::from_le_bytes(header[56..64].try_into()?);
    let compressed_bytes = u64::from_le_bytes(header[64..72].try_into()?);
    let block_positions = u32::from_le_bytes(header[120..124].try_into()?);
    let depth_kind = u32::from_le_bytes(header[124..128].try_into()?);
    if block_positions == 0 {
        return Err("invalid measurement block position count".into());
    }
    let blocks = positions.div_ceil(u64::from(block_positions));
    let mut largest_compressed_block_bytes = 0;
    file.seek(SeekFrom::Start(index_offset))?;
    for _ in 0..blocks {
        let mut row = [0; 56];
        file.read_exact(&mut row)?;
        largest_compressed_block_bytes =
            largest_compressed_block_bytes.max(u32::from_le_bytes(row[40..44].try_into()?));
    }
    let file_bytes = fs::metadata(path)?.len();
    Ok(json!({
        "base_end": base_end, "file_bytes": file_bytes, "log_bytes": file_bytes - base_end,
        "block_positions": block_positions, "depth_kind": depth_kind, "blocks": blocks,
        "block_index_offset": index_offset, "block_index_bytes": blocks * 56,
        "blocks_offset": blocks_offset, "compressed_bytes": compressed_bytes,
        "largest_compressed_block_bytes": largest_compressed_block_bytes
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Fixture {
    rule: String,
    count: u64,
    seed: u64,
    comments: bool,
}

impl Fixture {
    fn new(count: u64, seed: u64, comments: bool) -> Result<Self> {
        if count == 0 || count > MAX_POSITIONS {
            return Err("fixture count must be between 1 and 100000000".into());
        }
        Ok(Self { rule: RULE.into(), count, seed, comments })
    }

    fn from_book(path: &Path) -> Result<Self> {
        let metadata = NativeBook::open(path)?.metadata()?;
        let fixture: Self = serde_json::from_str(
            metadata.description.as_deref().ok_or("missing fixture metadata")?,
        )?;
        if fixture.rule != RULE {
            return Err("unrecognized fixture generation rule".into());
        }
        Self::new(fixture.count, fixture.seed, fixture.comments)
    }

    fn permutation(&self, index: u64) -> u64 {
        let mut multiplier = (self.seed % (POSITION_SPACE / 2)) * 2 + 1;
        if multiplier.is_multiple_of(3) {
            multiplier += 2;
        }
        let offset = mix64(self.seed ^ 0x7465_726e_6172_7933) % POSITION_SPACE;
        (index * multiplier + offset) % POSITION_SPACE
    }

    fn position(&self, index: u64, hit: bool) -> Result<Position> {
        if index >= self.count {
            return Err("fixture position index is out of range".into());
        }
        let mut number = self.permutation(index);
        let mut squares = [[b' '; 9]; 9];
        squares[0].copy_from_slice(b"lnsgkgsnl");
        squares[8].copy_from_slice(b"LNSGKGSNL");
        let digits: [usize; 18] = std::array::from_fn(|_| {
            let digit = (number % 3) as usize;
            number /= 3;
            digit
        });
        for (file, &rank) in digits[..9].iter().enumerate() {
            squares[1 + rank][file] = b'p';
        }
        for (file, &rank) in digits[9..].iter().enumerate() {
            squares[5 + rank][file] = b'P';
        }
        let mut sfen = String::with_capacity(110);
        for (rank, row) in squares.iter().enumerate() {
            if rank != 0 {
                sfen.push('/');
            }
            let mut empty = 0;
            for &piece in row {
                if piece == b' ' {
                    empty += 1;
                } else {
                    if empty != 0 {
                        sfen.push(char::from(b'0' + empty));
                        empty = 0;
                    }
                    sfen.push(char::from(piece));
                }
            }
            if empty != 0 {
                sfen.push(char::from(b'0' + empty));
            }
        }
        sfen.push_str(if hit { " b RBrb 1" } else { " w RBrb 1" });
        Ok(Position::from_sfen(&sfen)?)
    }

    fn record(&self, index: u64) -> Result<NativeBookPosition> {
        let position = self.position(index, true)?;
        position
            .validate()
            .map_err(|error| format!("invalid fixture position {index}: {error:?}"))?;
        let mut moves = MoveList::new();
        generate_moves::<LegalAll>(&position, &mut moves);
        let mut candidates = Vec::with_capacity(4);
        for (rank, &mv) in moves.iter().take(4).enumerate() {
            if !position.is_legal_move(mv) {
                return Err("fixture move generator returned illegal move".into());
            }
            let mut candidate = NativeCandidate::new(mv);
            candidate.score =
                NativeScore::Cp { value: 100 - rank as i32 * 7, bound: NativeBound::Exact };
            candidate.depth = Some(16);
            candidate.display_order = rank as u16;
            candidates.push(candidate);
        }
        if candidates.len() != 4 {
            return Err("fixture position has fewer than four legal moves".into());
        }
        let entry = NativeBookEntry::from_position(&position, candidates)?;
        let mut record = NativeBookPosition::new(entry);
        if self.comments && index.is_multiple_of(10) {
            record.comment = Some("c".repeat(100));
        }
        Ok(record)
    }
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        mix64(self.0)
    }

    fn below(&mut self, bound: u64) -> u64 {
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let value = self.next();
            if value >= threshold {
                return value % bound;
            }
        }
    }
}

struct Hash(u64);

impl Hash {
    fn new() -> Self {
        Self(FNV_OFFSET)
    }

    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(FNV_PRIME);
        }
    }

    fn entry(&mut self, entry: &NativeBookEntry) {
        self.update(&entry.key);
        self.update(&(entry.candidates.len() as u64).to_le_bytes());
        for candidate in &entry.candidates {
            self.update(&candidate.mv.raw().to_le_bytes());
            let (kind, bound, value): (u8, u8, i32) = match candidate.score {
                NativeScore::Unknown => (0, 0, 0),
                NativeScore::Cp { value, bound } => (1, bound_number(bound), value),
                NativeScore::Mate { plies, bound } => (2, bound_number(bound), plies),
                NativeScore::MateUnknown { winning } => (3, 0, if winning { 1 } else { -1 }),
            };
            self.update(&[kind, bound]);
            self.update(&value.to_le_bytes());
            self.update(&[u8::from(candidate.depth.is_some())]);
            self.update(&candidate.depth.unwrap_or(0).to_le_bytes());
            self.update(&candidate.display_order.to_le_bytes());
        }
    }

    fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

fn bound_number(bound: NativeBound) -> u8 {
    match bound {
        NativeBound::Exact => 0,
        NativeBound::Lower => 1,
        NativeBound::Upper => 2,
    }
}

fn generate(path: &Path, fixture: Fixture) -> Result<()> {
    let baseline_memory = memory_snapshot()?;
    let metadata = NativeBookMetadata {
        title: Some(RULE.into()),
        description: Some(serde_json::to_string(&fixture)?),
        depth_kind: 1,
    };
    let mut builder = NativeBookBuilder::new(path, metadata)?;
    let started = Instant::now();
    let mut hash = Hash::new();
    hash.update(RULE.as_bytes());
    hash.update(&fixture.seed.to_le_bytes());
    hash.update(&fixture.count.to_le_bytes());
    hash.update(&[u8::from(fixture.comments)]);
    for index in 0..fixture.count {
        let record = fixture.record(index)?;
        hash.entry(&record.entry);
        let comment = record.comment.as_deref().unwrap_or("");
        hash.update(&(comment.len() as u64).to_le_bytes());
        hash.update(comment.as_bytes());
        builder.push(record)?;
        if (index + 1) % 1_000_000 == 0 {
            eprintln!(
                "generated={} elapsed_seconds={:.3}",
                index + 1,
                started.elapsed().as_secs_f64()
            );
        }
    }
    builder.finish()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "command": "generate", "fixture": fixture,
            "path": path, "file_bytes": fs::metadata(path)?.len(),
            "elapsed_seconds": started.elapsed().as_secs_f64(),
            "builder_memory_budget_bytes": 256 * 1024 * 1024,
            "input_hash_algorithm": "fnv1a64", "input_hash": hash.hex(),
            "source_order": "fixture_index_ascending", "candidates_per_position": 4,
            "comment_rule": "100 ASCII bytes at fixture index divisible by ten when enabled",
        "qualification": "synthetic fixture; no reachability or playing-strength claim",
        "memory": memory_difference(&baseline_memory, &memory_snapshot()?)
        }))?
    );
    Ok(())
}

fn queries(fixture: &Fixture, count: usize) -> Result<Vec<(u64, bool)>> {
    if count == 0 || !count.is_multiple_of(2) {
        return Err("query count must be positive and even".into());
    }
    let mut random = Random(LOOKUP_SEED ^ fixture.seed);
    let mut queries: Vec<_> =
        (0..count).map(|index| (random.below(fixture.count), index % 2 == 0)).collect();
    for index in (1..count).rev() {
        let selected = random.below(index as u64 + 1) as usize;
        queries.swap(index, selected);
    }
    Ok(queries)
}

fn percentile(values: &mut [u64], percentile: usize) -> f64 {
    values.sort_unstable();
    let index = (values.len() * percentile).div_ceil(100) - 1;
    values[index] as f64 / 1_000_000.0
}

fn query_pass(
    book: &NativeBook,
    fixture: &Fixture,
    queries: &[(u64, bool)],
    measured: bool,
) -> Result<Value> {
    let mut latencies = Vec::with_capacity(if measured { queries.len() } else { 0 });
    let mut result_hash = Hash::new();
    let before = book.io_stats();
    let mut hits = 0;
    let mut max_calls_hit = 0;
    let mut max_calls_miss = 0;
    let mut max_bytes_hit = 0;
    let mut max_bytes_miss = 0;
    for &(index, hit) in queries {
        let position = fixture.position(index, hit)?;
        let query_before = book.io_stats();
        let started = Instant::now();
        let entry = book.get(&position)?;
        let elapsed = started.elapsed();
        let query_after = book.io_stats();
        if measured {
            latencies.push(u64::try_from(elapsed.as_nanos())?);
        }
        if entry.is_some() != hit {
            return Err(format!("fixture hit/miss mismatch at index {index}").into());
        }
        let calls = query_after.read_calls - query_before.read_calls;
        let bytes = query_after.read_bytes - query_before.read_bytes;
        if hit {
            hits += 1;
            max_calls_hit = max_calls_hit.max(calls);
            max_bytes_hit = max_bytes_hit.max(bytes);
        } else {
            max_calls_miss = max_calls_miss.max(calls);
            max_bytes_miss = max_bytes_miss.max(bytes);
        }
        result_hash.update(&index.to_le_bytes());
        result_hash.update(&[u8::from(hit)]);
        if let Some(entry) = entry {
            if entry.candidates.len() != 4 {
                return Err("fixture hit has unexpected candidate count".into());
            }
            result_hash.entry(&entry);
        }
    }
    let after = book.io_stats();
    let total_ns: u128 = latencies.iter().map(|&time| u128::from(time)).sum();
    Ok(json!({
        "queries": queries.len(), "hits": hits, "misses": queries.len() - hits,
        "lookup_p50_ms": measured.then(|| percentile(&mut latencies, 50)),
        "lookup_p95_ms": measured.then(|| percentile(&mut latencies, 95)),
        "lookup_p99_ms": measured.then(|| percentile(&mut latencies, 99)),
        "lookup_max_ms": latencies.last().map(|&time| time as f64 / 1_000_000.0),
        "lookup_mean_ms": measured.then_some(total_ns as f64 / queries.len() as f64 / 1_000_000.0),
        "read_at_calls": after.read_calls - before.read_calls,
        "read_requested_bytes": after.read_bytes - before.read_bytes,
        "file_seek_calls": after.seek_calls - before.seek_calls,
        "file_read_calls": after.read_syscalls - before.read_syscalls,
        "file_call_counter_boundary": "standard File seek/read API calls, including read retries; not a kernel trace or physical I/O count",
        "max_read_at_calls_hit": max_calls_hit, "max_read_at_calls_miss": max_calls_miss,
        "max_read_requested_bytes_hit": max_bytes_hit, "max_read_requested_bytes_miss": max_bytes_miss,
        "comment_api_requests": 0, "lookup_api": "NativeBook::get",
        "result_hash_algorithm": "fnv1a64", "result_hash": result_hash.hex()
    }))
}

fn worker(path: &Path, query_count: usize) -> Result<Value> {
    let fixture = Fixture::from_book(path)?;
    let queries = queries(&fixture, query_count)?;
    let baseline_memory = memory_snapshot()?;
    let warmup = NativeBook::open(path)?;
    query_pass(&warmup, &fixture, &queries, false)?;
    drop(warmup);
    let started = Instant::now();
    let book = NativeBook::open(path)?;
    let open_ns = u64::try_from(started.elapsed().as_nanos())?;
    let open_io = book.io_stats();
    let mut result = query_pass(&book, &fixture, &queries, true)?;
    result["pid"] = json!(std::process::id());
    result["open_ns"] = json!(open_ns);
    result["open_ms"] = json!(open_ns as f64 / 1_000_000.0);
    result["open_read_at_calls"] = json!(open_io.read_calls);
    result["open_read_requested_bytes"] = json!(open_io.read_bytes);
    result["open_file_seek_calls"] = json!(open_io.seek_calls);
    result["open_file_read_calls"] = json!(open_io.read_syscalls);
    let layout = file_layout(path)?;
    let log_bytes = layout["log_bytes"].as_u64().ok_or("missing log size")?;
    let index_bytes = layout["block_index_bytes"].as_u64().ok_or("missing block index size")?;
    let largest_block =
        layout["largest_compressed_block_bytes"].as_u64().ok_or("missing compressed block size")?;
    result["layout"] = layout;
    result["open_log_bytes_per_second"] = if log_bytes == 0 {
        Value::Null
    } else {
        json!(log_bytes as f64 / (open_ns as f64 / 1_000_000_000.0))
    };
    result["memory"] = memory_difference(&baseline_memory, &memory_snapshot()?);
    let memory_limit = if log_bytes == 0 { 128 * 1024 * 1024 } else { 256 * 1024 * 1024 };
    // 差分 snapshot は候補手ヘッダーと 4 候補の固定長領域を読む。
    let lookup_calls_limit = if log_bytes == 0 { 1 } else { 2 };
    let lookup_bytes_limit =
        if log_bytes == 0 { largest_block } else { largest_block.max(16 + 4 * 16) };
    result["gates"] = json!({
        "open_within_2_seconds": open_ns <= 2_000_000_000,
        "lookup_p95_within_5_ms": result["lookup_p95_ms"].as_f64().is_some_and(|ms| ms <= 5.0),
        "additional_peak_private_within_limit": result["memory"]["additional_peak_private_upper_bound_bytes"].as_u64().map(|bytes| bytes <= memory_limit),
        "additional_peak_private_limit_bytes": memory_limit,
        "bounded_hit_read_calls": result["max_read_at_calls_hit"].as_u64().is_some_and(|calls| calls <= lookup_calls_limit),
        "bounded_miss_read_calls": result["max_read_at_calls_miss"].as_u64().is_some_and(|calls| calls <= 1),
        "bounded_hit_read_bytes": result["max_read_requested_bytes_hit"].as_u64().is_some_and(|bytes| bytes <= lookup_bytes_limit),
        "bounded_miss_read_bytes": result["max_read_requested_bytes_miss"].as_u64().is_some_and(|bytes| bytes <= largest_block),
        "comment_api_requests_zero": true,
        "largest_compressed_block_bytes": largest_block,
        "open_reads_header_and_block_index_when_no_log": log_bytes != 0 || (open_io.read_calls <= 2 && open_io.read_bytes == 128 + index_bytes)
    });
    Ok(result)
}

fn measure(path: &Path, repeats: usize, output: &Path, query_count: usize) -> Result<()> {
    if repeats == 0 {
        return Err("measurement repeats must be positive".into());
    }
    let fixture = Fixture::from_book(path)?;
    let _ = queries(&fixture, query_count)?;
    let executable = std::env::current_exe()?;
    let mut results = Vec::with_capacity(repeats);
    let mut times = Vec::with_capacity(repeats);
    for repetition in 0..repeats {
        let child = Command::new(&executable)
            .arg("worker")
            .arg(path)
            .arg(query_count.to_string())
            .output()?;
        if !child.status.success() {
            return Err(format!(
                "measurement worker {repetition} failed: {}",
                String::from_utf8_lossy(&child.stderr)
            )
            .into());
        }
        let result: Value = serde_json::from_slice(&child.stdout)?;
        times.push(result["open_ns"].as_u64().ok_or("worker omitted open time")?);
        if results
            .first()
            .is_some_and(|first: &Value| first["result_hash"] != result["result_hash"])
        {
            return Err("measurement workers produced inconsistent result hashes".into());
        }
        results.push(result);
    }
    let report = json!({
        "command": "measure", "fixture": fixture, "path": path,
        "file_bytes": fs::metadata(path)?.len(), "repeats": repeats,
        "qualifying_repetition_count": repeats >= 5,
        "cache_condition": "warm workload in each independent worker; OS cache not evicted",
        "build_condition": if cfg!(debug_assertions) { "debug; not qualifying" } else { "release" },
        "target_os": std::env::consts::OS, "target_arch": std::env::consts::ARCH,
        "lookup_seed": LOOKUP_SEED ^ fixture.seed,
        "open_p95_ms": percentile(&mut times, 95),
        "lookup_p95_ms_worst_repetition": results.iter().filter_map(|result| result["lookup_p95_ms"].as_f64()).reduce(f64::max),
        "lookup_p95_ms_reporting": "per-repetition nearest-rank percentiles; aggregate is worst repetition",
        "comment_api_requests": 0,
        "memory_and_physical_io": "measure separately; reader counters are requested application I/O",
        "results": results
    });
    write_report(output, &report)
}

fn copy_new(source: &Path, destination: &Path) -> Result<()> {
    let mut input = std::fs::File::open(source)?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(destination)?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let len = input.read(&mut buffer)?;
        if len == 0 {
            break;
        }
        output.write_all(&buffer[..len])?;
    }
    output.sync_all()?;
    Ok(())
}

fn edits(
    source: &Path,
    destination: &Path,
    count: u64,
    repeat_position: bool,
    output: &Path,
) -> Result<()> {
    let fixture = Fixture::from_book(source)?;
    if count == 0 || (!repeat_position && count > fixture.count) {
        return Err("invalid edit count".into());
    }
    copy_new(source, destination)?;
    let baseline = memory_snapshot()?;
    let mut editor = NativeBookEditor::open(destination)?;
    let initial_metadata = editor.metadata()?;
    let before = fs::metadata(destination)?.len();
    let mut save_ns = Vec::with_capacity(usize::try_from(count)?);
    let mut append_bytes = Vec::with_capacity(usize::try_from(count)?);
    let started = Instant::now();
    for index in 0..count {
        let mut record = fixture.record(if repeat_position { 0 } else { index })?;
        record.comment = Some("e".repeat(1024));
        // 同じ局面の反復でも各 snapshot を明示的に変更し、no-op 保存を避ける。
        record.entry.candidates[0].depth =
            Some(u32::try_from(index)?.checked_add(17).ok_or("edit depth overflow")?);
        editor.upsert_position(record)?;
        let length_before = fs::metadata(destination)?.len();
        let save_started = Instant::now();
        if !editor.save()? {
            return Err("benchmark edit unexpectedly became a no-op".into());
        }
        save_ns.push(u64::try_from(save_started.elapsed().as_nanos())?);
        append_bytes.push(fs::metadata(destination)?.len() - length_before);
    }
    if editor.metadata()? != initial_metadata {
        return Err("edit workload changed document metadata".into());
    }
    let report = json!({
        "command": "edits", "source": source, "destination": destination, "fixture": fixture,
        "transactions": count, "different_positions": if repeat_position { 1 } else { count },
        "candidates_per_edit": 4, "comment_bytes_per_edit": 1024,
        "elapsed_seconds": started.elapsed().as_secs_f64(),
        "save_p95_ms": percentile(&mut save_ns, 95),
        "save_max_ms": *save_ns.iter().max().ok_or("empty save timings")? as f64 / 1_000_000.0,
        "append_bytes_min": append_bytes.iter().min(), "append_bytes_max": append_bytes.iter().max(),
        "total_append_bytes": fs::metadata(destination)?.len() - before,
        "file_layout": file_layout(destination)?,
        "memory": memory_difference(&baseline, &memory_snapshot()?),
        "metadata_preserved": true,
        "gates": {
            "save_p95_within_1_second": percentile(&mut save_ns, 95) <= 1000.0,
            "append_per_save_within_4_kib": append_bytes.iter().all(|&bytes| bytes <= 4096)
        }
    });
    write_report(output, &report)
}

fn logical_hash(book: &NativeBook) -> Result<Value> {
    let started = Instant::now();
    let mut hash = Hash::new();
    let metadata = book.metadata()?;
    hash.update(&metadata.depth_kind.to_le_bytes());
    for text in [metadata.title, metadata.description] {
        let text = text.unwrap_or_default();
        hash.update(&(text.len() as u64).to_le_bytes());
        hash.update(text.as_bytes());
    }
    let mut positions = 0u64;
    let mut candidates = 0u64;
    for record in book.records() {
        let record = record?;
        hash.entry(&record.entry);
        let comment = record.comment.as_deref().unwrap_or("");
        hash.update(&(comment.len() as u64).to_le_bytes());
        hash.update(comment.as_bytes());
        hash.update(&(record.move_comments.len() as u64).to_le_bytes());
        for (mv, text) in record.move_comments {
            hash.update(&mv.to_le_bytes());
            hash.update(&(text.len() as u64).to_le_bytes());
            hash.update(text.as_bytes());
        }
        positions += 1;
        candidates += record.entry.candidates.len() as u64;
    }
    Ok(
        json!({"algorithm": "fnv1a64", "hash": hash.hex(), "positions": positions, "candidates": candidates, "elapsed_seconds": started.elapsed().as_secs_f64()}),
    )
}

fn validate(path: &Path, output: &Path) -> Result<()> {
    let baseline = memory_snapshot()?;
    let book = NativeBook::open(path)?;
    let started = Instant::now();
    book.validate_full()?;
    let seconds = started.elapsed().as_secs_f64();
    let hash = logical_hash(&book)?;
    write_report(
        output,
        &json!({"command": "validate", "path": path, "elapsed_seconds": seconds,
        "logical": hash, "layout": file_layout(path)?, "memory": memory_difference(&baseline, &memory_snapshot()?), "validated_full": true}),
    )
}

fn compact(source: &Path, destination: &Path, output: &Path) -> Result<()> {
    copy_new(source, destination)?;
    let baseline = memory_snapshot()?;
    let mut editor = NativeBookEditor::open(destination)?;
    let before = logical_hash(editor.book())?;
    let started = Instant::now();
    editor.compact()?;
    let elapsed_seconds = started.elapsed().as_secs_f64();
    let compact_memory = memory_difference(&baseline, &memory_snapshot()?);
    let after = logical_hash(editor.book())?;
    let equal = before["hash"] == after["hash"] && before["positions"] == after["positions"];
    if !equal {
        return Err("compaction changed logical contents".into());
    }
    write_report(
        output,
        &json!({"command": "compact", "source": source, "destination": destination,
        "elapsed_seconds": elapsed_seconds, "before": before, "after": after, "logical_equal": equal,
        "layout": file_layout(destination)?, "memory": compact_memory,
        "memory_gate_within_256_mib": compact_memory["additional_peak_private_upper_bound_bytes"].as_u64().map(|bytes| bytes <= 256 * 1024 * 1024)}),
    )
}

fn sorted(source: &Path, destination: &Path, output: &Path) -> Result<()> {
    let baseline = memory_snapshot()?;
    let editor = NativeBookEditor::open(source)?;
    let started = Instant::now();
    editor.write_to(destination)?;
    let elapsed_seconds = started.elapsed().as_secs_f64();
    let memory = memory_difference(&baseline, &memory_snapshot()?);
    let before = logical_hash(editor.book())?;
    let after = logical_hash(&NativeBook::open(destination)?)?;
    if before["hash"] != after["hash"] {
        return Err("sorted build changed logical contents".into());
    }
    write_report(
        output,
        &json!({"command": "sorted", "source": source, "destination": destination,
        "elapsed_seconds": elapsed_seconds, "memory": memory, "before": before, "after": after,
        "logical_equal": true, "memory_gate_within_256_mib": memory["additional_peak_private_upper_bound_bytes"].as_u64().map(|bytes| bytes <= 256 * 1024 * 1024)}),
    )
}

fn main() -> Result<()> {
    board::init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("generate") if matches!(args.len(), 4 | 5) => {
            if args.len() == 5 && args[4] != "--comments" { return Err("unknown generate option".into()); }
            generate(Path::new(&args[1]), Fixture::new(args[2].parse()?, args[3].parse()?, args.len() == 5)?)
        }
        Some("measure") if matches!(args.len(), 4 | 6) => {
            let query_count = if args.len() == 6 {
                if args[4] != "--queries" { return Err("unknown measure option".into()); }
                args[5].parse()?
            } else { DEFAULT_QUERIES };
            measure(Path::new(&args[1]), args[2].parse()?, Path::new(&args[3]), query_count)
        }
        Some("worker") if args.len() == 3 => {
            println!("{}", worker(Path::new(&args[1]), args[2].parse()?)?);
            Ok(())
        }
        Some("edits") if matches!(args.len(), 5 | 6) => {
            let repeat = args.len() == 6;
            if repeat && args[5] != "--repeat-position" { return Err("unknown edit option".into()); }
            edits(Path::new(&args[1]), Path::new(&args[2]), args[3].parse()?, repeat, Path::new(&args[4]))
        }
        Some("validate") if args.len() == 3 => validate(Path::new(&args[1]), Path::new(&args[2])),
        Some("compact") if args.len() == 4 => compact(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3])),
        Some("sorted") if args.len() == 4 => sorted(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3])),
        _ => Err("usage: native_book_measure generate PATH COUNT SEED [--comments] | measure PATH REPEATS OUTPUT_JSON [--queries EVEN_COUNT] | edits SOURCE DEST COUNT OUTPUT_JSON [--repeat-position] | validate PATH OUTPUT_JSON | compact SOURCE DEST OUTPUT_JSON | sorted SOURCE DEST OUTPUT_JSON".into()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn first_thousand_keys_are_unique_legal_and_have_four_moves() {
        board::init();
        for seed in [0, 1, 42, u64::MAX] {
            let fixture = Fixture::new(1000, seed, true).unwrap();
            let mut keys = HashSet::new();
            let mut comments = 0;
            for index in 0..fixture.count {
                let record = fixture.record(index).unwrap();
                assert!(keys.insert(record.entry.key));
                assert_eq!(record.entry.candidates.len(), 4);
                let position = record.entry.position().unwrap();
                assert!(position.validate().is_ok());
                assert!(!position.is_in_check());
                for candidate in record.entry.candidates {
                    assert!(position.is_legal_move(candidate.mv));
                }
                let miss_key =
                    fixture.position(index, false).unwrap().try_to_packed_sfen().unwrap();
                assert_ne!(&record.entry.key, miss_key.as_bytes());
                if let Some(comment) = record.comment {
                    comments += 1;
                    assert_eq!(comment.len(), 100);
                }
            }
            assert_eq!(comments, 100);
            assert_eq!(keys.len(), 1000);
        }
    }

    #[test]
    fn workload_is_deterministic_balanced_and_bounded() {
        let fixture = Fixture::new(1000, 42, false).unwrap();
        let first = queries(&fixture, 1000).unwrap();
        assert_eq!(first, queries(&fixture, 1000).unwrap());
        assert_eq!(first.iter().filter(|(_, hit)| *hit).count(), 500);
        assert!(first.iter().all(|(index, _)| *index < 1000));
        assert!(queries(&fixture, 0).is_err());
        assert!(queries(&fixture, 999).is_err());
        assert!(Fixture::new(0, 0, false).is_err());
        assert!(Fixture::new(MAX_POSITIONS + 1, 0, false).is_err());
        let mut values = [1_000_000, 5_000_000, 3_000_000, 2_000_000, 4_000_000];
        assert_eq!(percentile(&mut values, 95), 5.0);
        assert_eq!(percentile(&mut values, 50), 3.0);
    }

    #[test]
    fn comment_edit_validate_compact_and_sorted_commands_preserve_contents() {
        board::init();
        let directory = std::env::temp_dir().join(format!(
            "rsshogi-native-measure-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let base = directory.join("base.book");
        let log = directory.join("log.book");
        let compacted = directory.join("compacted.book");
        let sorted_path = directory.join("sorted.book");
        generate(&base, Fixture::new(32, 42, true).unwrap()).unwrap();
        edits(&base, &log, 10, false, &directory.join("edits.json")).unwrap();
        let edit_report: Value =
            serde_json::from_slice(&fs::read(directory.join("edits.json")).unwrap()).unwrap();
        assert_eq!(edit_report["different_positions"], 10);
        assert_eq!(edit_report["append_bytes_min"], 1264);
        assert_eq!(edit_report["append_bytes_max"], 1264);
        validate(&log, &directory.join("validate.json")).unwrap();
        compact(&log, &compacted, &directory.join("compact.json")).unwrap();
        sorted(&base, &sorted_path, &directory.join("sorted.json")).unwrap();
        let compact_report: Value =
            serde_json::from_slice(&fs::read(directory.join("compact.json")).unwrap()).unwrap();
        assert_eq!(compact_report["logical_equal"], true);
        assert_eq!(compact_report["layout"]["log_bytes"], 0);
        let sample = worker(&base, 20).unwrap();
        assert_eq!(sample["hits"], 10);
        assert_eq!(sample["misses"], 10);
        assert_eq!(sample["comment_api_requests"], 0);
        assert_eq!(sample["open_file_seek_calls"], 2);
        assert_eq!(sample["open_file_read_calls"], 2);
        for name in [
            "base.book",
            "log.book",
            "compacted.book",
            "sorted.book",
            "edits.json",
            "validate.json",
            "compact.json",
            "sorted.json",
            "log.book.lock",
            "base.book.lock",
            "compacted.book.lock",
            "sorted.book.lock",
        ] {
            fs::remove_file(directory.join(name)).unwrap();
        }
        fs::remove_dir(directory).unwrap();
    }
}
