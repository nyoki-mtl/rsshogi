//! YBB 全件走査の時間と内容照合用 fingerprint を計測する開発用ツール。

#[cfg(feature = "book")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Instant;

    use rsshogi::book::{YbbBook, YbbBookOpenOptions, YbbStorageMode, YbbValidationMode};

    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: ybb_scan_measure PATH [on-access|full] [LIMIT]")?;
    let validation = match args.next().as_deref().unwrap_or("on-access") {
        "on-access" => YbbValidationMode::OnAccess,
        "full" => YbbValidationMode::Full,
        _ => return Err("unknown validation mode".into()),
    };
    let limit: usize = args.next().map(|value| value.parse()).transpose()?.unwrap_or(usize::MAX);
    let start = Instant::now();
    let book = YbbBook::open_with_options(
        &path,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(validation),
    )?;
    let open_seconds = start.elapsed().as_secs_f64();
    let scan = Instant::now();
    let mut row_count = 0u64;
    let mut move_count = 0u64;
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |value: u64| {
        fingerprint = (fingerprint ^ value).wrapping_mul(0x100_0000_01b3);
    };
    for entry in book.iter_entries().take(limit) {
        let entry = entry?;
        for chunk in entry.packed_sfen().as_bytes().chunks_exact(8) {
            mix(u64::from_le_bytes(chunk.try_into()?));
        }
        mix(u64::from(entry.ply()));
        mix(entry.moves().len() as u64);
        for candidate in entry.moves() {
            mix(u64::from(candidate.mv().raw()));
            mix(u64::from(candidate.raw_move()));
            mix(candidate.eval() as i64 as u64);
            mix(candidate.depth().map_or(u64::MAX, u64::from));
        }
        row_count += 1;
        move_count += entry.moves().len() as u64;
    }
    println!(
        "{{\"total_rows\":{},\"rows\":{row_count},\"moves\":{move_count},\"fingerprint\":\"{fingerprint:016x}\",\"open_seconds\":{open_seconds:.6},\"scan_seconds\":{:.6},\"total_seconds\":{:.6}}}",
        book.len(),
        scan.elapsed().as_secs_f64(),
        start.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(not(feature = "book"))]
fn main() {
    eprintln!("ybb_scan_measure requires --features book");
}
