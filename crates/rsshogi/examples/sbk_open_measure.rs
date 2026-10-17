//! `SbkBook::open` の wall time とピークメモリの計測。
//!
//! 計測用の SBK は平手から幅優先で合成するため、外部 fixture を必要としない。
//! 生成とオープンはピークメモリが混ざらないよう別サブコマンドに分けてある。
//!
//! ```text
//! sbk_open_measure gen <out.sbk> <state_count> [branching]
//! sbk_open_measure open <path> [reps]
//! ```

use std::collections::{HashMap, VecDeque};
use std::env;
use std::error::Error;
use std::fs;
use std::time::Instant;

use rsshogi::board::{self, InitialPosition, MoveList, Position};
use rsshogi::board::{LegalAll, generate_moves};
use rsshogi::book::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata,
    BookPosition, SbkBook, SbkWriteOptions,
};

fn main() -> Result<(), Box<dyn Error>> {
    board::init();
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("gen") => {
            let path = args.get(1).ok_or("gen requires <out.sbk>")?;
            let state_count: usize = args.get(2).ok_or("gen requires <state_count>")?.parse()?;
            let branching: usize =
                args.get(3).map_or(Ok(5), |value| value.parse()).map_err(Box::new)?;
            generate(path, state_count, branching)
        }
        Some("open") => {
            let path = args.get(1).ok_or("open requires <path>")?;
            let reps: usize = args.get(2).map_or(Ok(1), |value| value.parse()).map_err(Box::new)?;
            measure_open(path, reps)
        }
        _ => Err("usage: sbk_open_measure gen <out.sbk> <state_count> [branching] \
                  | open <path> [reps]"
            .into()),
    }
}

/// 平手から幅優先で合成した定跡グラフを SBK として書き出す。
fn generate(path: &str, state_count: usize, branching: usize) -> Result<(), Box<dyn Error>> {
    let root = Position::from_sfen(InitialPosition::Standard.to_sfen())?.to_sfen(Some(1));
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut queue = VecDeque::new();
    let mut entries = Vec::with_capacity(state_count);
    let mut moves = MoveList::default();

    seen.insert(root.clone(), ());
    queue.push_back(root);

    while let Some(sfen) = queue.pop_front() {
        if entries.len() >= state_count {
            break;
        }
        let mut position = Position::from_sfen(&sfen)?;
        generate_moves::<LegalAll>(&position, &mut moves);
        let selected: Vec<_> = moves.iter().copied().take(branching).collect();
        if selected.is_empty() {
            continue;
        }

        let mut candidates = Vec::with_capacity(selected.len());
        for (rank, mv) in selected.iter().copied().enumerate() {
            let mv32 = position.move32_from_move(mv);
            position.apply_move32(mv32);
            let child = position.to_sfen(Some(1));
            position.undo_move32(mv32).map_err(|err| format!("undo failed: {err:?}"))?;

            // 評価値と weight を埋めて、実在の定跡に近いペイロードサイズにする。
            let score = 100 - i32::try_from(rank)? * 7;
            candidates.push(BookCandidate::new(
                mv,
                None,
                Some(score),
                Some(16),
                Some(1000 - i32::try_from(rank)? * 100),
                None,
                BookMoveMetadata::new(),
            ));

            if seen.len() < state_count && !seen.contains_key(&child) {
                seen.insert(child.clone(), ());
                queue.push_back(child);
            }
        }

        entries.push(BookDatabaseEntry::new(
            BookPosition::from_sfen(&sfen, None)?,
            candidates,
            BookEntryMetadata::new(),
        ));
    }

    let database = BookDatabase::try_from_entries(entries)?;
    let states = database.entries().len();
    let bytes = database.to_sbk_bytes(&SbkWriteOptions::new())?;
    fs::write(path, &bytes)?;

    println!("path={path}");
    println!("states={states}");
    println!("file_bytes={}", bytes.len());
    println!("size_of_position={}", size_of::<Position>());
    Ok(())
}

/// `SbkBook::open` を `reps` 回実行し、各回の経過時間とプロセスのピーク値を報告する。
fn measure_open(path: &str, reps: usize) -> Result<(), Box<dyn Error>> {
    let file_bytes = fs::metadata(path)?.len();
    let baseline = peak_working_set();
    println!("path={path}");
    println!("file_bytes={file_bytes}");
    println!("baseline_peak_bytes={baseline}");

    for rep in 0..reps {
        let started = Instant::now();
        let book = SbkBook::open(path)?;
        let elapsed = started.elapsed();
        // open の戻り値が最適化で消えないよう、インデックス件数を読み出してから drop する。
        let indexed = book.len();
        drop(book);
        let peak = peak_working_set();
        println!(
            "rep={rep} open_ms={:.3} indexed={indexed} peak_bytes={peak} peak_delta_bytes={}",
            elapsed.as_secs_f64() * 1000.0,
            peak.saturating_sub(baseline)
        );
    }
    Ok(())
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCounters {
    cb: u32,
    page_fault_count: u32,
    peak_working_set_size: usize,
    working_set_size: usize,
    quota_peak_paged_pool_usage: usize,
    quota_paged_pool_usage: usize,
    quota_peak_non_paged_pool_usage: usize,
    quota_non_paged_pool_usage: usize,
    pagefile_usage: usize,
    peak_pagefile_usage: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentProcess() -> *mut core::ffi::c_void;
    fn K32GetProcessMemoryInfo(
        process: *mut core::ffi::c_void,
        counters: *mut ProcessMemoryCounters,
        cb: u32,
    ) -> i32;
}

/// 自プロセスのピークワーキングセットをバイト単位で返す。取得できなければ 0。
#[cfg(windows)]
fn peak_working_set() -> u64 {
    let mut counters = ProcessMemoryCounters {
        cb: u32::try_from(size_of::<ProcessMemoryCounters>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: counters は自身のサイズを cb として渡す有効な書き込み先であり、
    // GetCurrentProcess の疑似ハンドルは常に有効で close 不要。
    let ok =
        unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &raw mut counters, counters.cb) };
    if ok == 0 { 0 } else { counters.peak_working_set_size as u64 }
}

#[cfg(not(windows))]
fn peak_working_set() -> u64 {
    0
}
