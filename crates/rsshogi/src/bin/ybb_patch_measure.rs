//! YBB の局面差分保存と段階別の所要時間を確認する開発用ツール。

#[cfg(feature = "book")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;
    use std::io::BufWriter;
    use std::time::Instant;

    use rsshogi::board::Position;
    use rsshogi::book::{
        BookCandidate, BookControl, BookDatabaseEntry, BookDatabasePatch, BookEntryMetadata,
        BookMoveMetadata, BookPatchWritePhase, BookPosition, YbbBook, YbbBookOpenOptions,
        YbbStorageMode, YbbValidationMode,
    };

    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .ok_or("usage: ybb_patch_measure INPUT NEW_OUTPUT [copy|delete-first|replace-first]")?;
    let output = args.next().ok_or("output is required")?;
    let mode = args.next().unwrap_or_else(|| "copy".to_owned());
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let book = YbbBook::open_with_options(
        &input,
        YbbBookOpenOptions::new()
            .with_storage_mode(YbbStorageMode::File)
            .with_validation_mode(YbbValidationMode::OnAccess),
    )?;
    let mut patch = BookDatabasePatch::new();
    match mode.as_str() {
        "copy" => {}
        "delete-first" | "replace-first" => {
            let first = book.iter_entries().next().ok_or("source is empty")??;
            let mut position = Position::empty();
            position.set_packed_sfen(&first.packed_sfen(), false, 1)?;
            if mode == "delete-first" {
                patch.remove_entry_by_sfen(&position.to_sfen(Some(1)))?;
            } else {
                let candidates = first
                    .moves()
                    .iter()
                    .enumerate()
                    .map(|(index, m)| {
                        let value = i32::from(m.eval());
                        let score = if index == 0 {
                            if value == i32::from(i16::MAX) { value - 1 } else { value + 1 }
                        } else {
                            value
                        };
                        BookCandidate::new(
                            m.mv(),
                            None,
                            Some(score),
                            m.depth().map(u32::from),
                            None,
                            None,
                            BookMoveMetadata::default(),
                        )
                    })
                    .collect();
                patch.upsert_entry(BookDatabaseEntry::new(
                    BookPosition::from_position(&position, Some(u32::from(first.ply()))),
                    candidates,
                    BookEntryMetadata::default(),
                ))?;
            }
        }
        _ => return Err("unknown patch mode".into()),
    }
    // 保存先は caller が所有し、原本や既存ファイルを上書きしない。
    let file = OpenOptions::new().write(true).create_new(true).open(output)?;
    let mut writer = BufWriter::new(file);
    let start = Instant::now();
    let mut validating_seconds = None;
    book.write_ybb_with_patch_with_control(&mut writer, &patch, |point| {
        if point.phase() == BookPatchWritePhase::Writing && validating_seconds.is_none() {
            validating_seconds = Some(start.elapsed().as_secs_f64());
        }
        BookControl::Continue
    })?;
    let total_seconds = start.elapsed().as_secs_f64();
    let validating_seconds = validating_seconds.ok_or("missing writing phase")?;
    println!(
        "{{\"mode\":\"{mode}\",\"source_rows\":{},\"validating_seconds\":{validating_seconds:.6},\"writing_seconds\":{:.6},\"total_seconds\":{total_seconds:.6}}}",
        book.len(),
        total_seconds - validating_seconds
    );
    Ok(())
}

#[cfg(not(feature = "book"))]
fn main() {
    eprintln!("ybb_patch_measure requires --features book");
}
