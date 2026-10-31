//! YBB の重複分布と省メモリ複製を確認する開発用ツール。

#[cfg(feature = "book")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;
    use std::io::BufWriter;
    use std::time::Instant;

    use rsshogi::book::{
        YbbBook, YbbBookOpenOptions, YbbStorageMode, YbbValidationMode, YbbWriteOptions, YbbWriter,
    };

    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: ybb_diagnose PATH [NEW_COPY_PATH]")?;
    let copy_path = args.next();
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let book = YbbBook::open_with_options(
        &path,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    )?;
    let start = Instant::now();
    let report = book.diagnose_full()?;
    let diagnose_seconds = start.elapsed().as_secs_f64();

    // 群サイズは floor(log2(size)) ごとに数え、全群を保持しない。
    let mut histogram = [0u64; 64];
    let mut previous = None;
    let mut group_rows = 0u64;
    let mut max_group_rows = 0u64;
    let mut sample_key = None;
    for entry in book.iter_entries() {
        let entry = entry?;
        if previous != Some(entry.packed_sfen()) {
            if group_rows != 0 {
                histogram[group_rows.ilog2() as usize] += 1;
                max_group_rows = max_group_rows.max(group_rows);
            }
            previous = Some(entry.packed_sfen());
            group_rows = 0;
        }
        group_rows += 1;
        if group_rows == 2 && sample_key.is_none() {
            sample_key = previous;
        }
    }
    if group_rows != 0 {
        histogram[group_rows.ilog2() as usize] += 1;
        max_group_rows = max_group_rows.max(group_rows);
    }
    let mut sample_plies = Vec::new();
    let mut sample_move_counts = Vec::new();
    let mut sample_moves_differ = false;
    if let Some(key) = sample_key {
        let mut rows = book.iter_entries_by_packed_sfen(key)?;
        if let Some(first) = rows.next() {
            let first = first?;
            sample_plies.push(first.ply());
            sample_move_counts.push(first.moves().len());
            for entry in rows.take(7) {
                let entry = entry?;
                sample_plies.push(entry.ply());
                sample_move_counts.push(entry.moves().len());
                sample_moves_differ |= entry.moves() != first.moves();
            }
        }
    }
    let distribution_seconds = start.elapsed().as_secs_f64() - diagnose_seconds;
    let mut copy_seconds = None;
    if let Some(output) = copy_path {
        let start = Instant::now();
        let file = OpenOptions::new().write(true).create_new(true).open(output)?;
        let mut writer = YbbWriter::new(
            BufWriter::new(file),
            book.len(),
            YbbWriteOptions::new().with_depth(book.has_depth()),
        )?;
        for entry in book.iter_entries() {
            writer.write_entry(&entry?)?;
        }
        writer.finish()?;
        copy_seconds = Some(start.elapsed().as_secs_f64());
    }
    println!(
        concat!(
            "{{\"rows\":{},\"positions\":{},\"duplicate_positions\":{},",
            "\"duplicate_rows\":{},\"duplicate_ply_rows\":{},\"varying_ply_positions\":{},",
            "\"has_depth\":{},\"max_group_rows\":{max_group_rows},",
            "\"group_size_log2_histogram\":{histogram:?},\"sample_plies\":{sample_plies:?},",
            "\"sample_move_counts\":{sample_move_counts:?},\"sample_moves_differ\":{sample_moves_differ},",
            "\"diagnose_seconds\":{diagnose_seconds:.6},\"distribution_seconds\":{distribution_seconds:.6},",
            "\"copy_seconds\":{}}}"
        ),
        report.row_count(),
        report.position_count(),
        report.duplicate_position_count(),
        report.duplicate_row_count(),
        report.duplicate_ply_row_count(),
        report.varying_ply_position_count(),
        book.has_depth(),
        copy_seconds.map_or_else(|| "null".to_owned(), |n| format!("{n:.6}")),
        max_group_rows = max_group_rows,
        histogram = histogram,
        sample_plies = sample_plies,
        sample_move_counts = sample_move_counts,
        sample_moves_differ = sample_moves_differ,
        diagnose_seconds = diagnose_seconds,
        distribution_seconds = distribution_seconds,
    );
    Ok(())
}

#[cfg(not(feature = "book"))]
fn main() {
    eprintln!("ybb_diagnose requires --features book");
}
