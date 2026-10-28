//! DB/SBK の逐次保存と全件 IR 保存を別プロセスで比較する開発用計測。

#[cfg(feature = "book")]
use rsshogi::{board, book::*};
#[cfg(feature = "book")]
use std::{env, fs::File, io::BufWriter, time::Instant};

#[cfg(feature = "book")]
fn change(entry: BookDatabaseEntry, sbk: bool) -> BookDatabaseEntry {
    let metadata = if sbk {
        let old = entry.metadata().sbk().expect("SBK metadata");
        BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
            old.state_id(),
            old.state_index(),
            old.games(),
            old.won_black(),
            old.won_white(),
            Some(format!("{} [patch-measure]", old.comment().unwrap_or_default())),
            old.evals().to_vec(),
        ))
    } else {
        let old = entry.metadata().yaneuraou().expect("DB metadata");
        BookEntryMetadata::from_yaneuraou(
            old.min_ply(),
            format!("{} [patch-measure]", old.comment()),
        )
    };
    BookDatabaseEntry::new(entry.position().clone(), entry.candidates().to_vec(), metadata)
}

#[cfg(feature = "book")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    board::init();
    let args: Vec<_> = env::args().collect();
    if args.get(1).map(String::as_str) == Some("fixtures") {
        return fixtures(args.get(2).ok_or("fixtures requires output directory")?);
    }
    if args.len() != 6 {
        return Err(
            "usage: book_patch_measure <sbk|db> <stream|full|verify> <input> <output> <changes>"
                .into(),
        );
    }
    let sbk = args[1] == "sbk";
    let mode = args[2].as_str();
    let count: usize = args[5].parse()?;
    let start = Instant::now();
    let mut entries = Vec::new();
    let mut patch = BookDatabasePatch::new();
    let mut writer = BufWriter::new(File::create(&args[4])?);
    let open_seconds;
    let mut scanned = 0;
    if sbk {
        let book = SbkBook::open(&args[3])?;
        open_seconds = start.elapsed().as_secs_f64();
        for (i, entry) in book.iter_entries().enumerate() {
            if mode == "stream" && i >= count {
                break;
            }
            let mut entry = BookDatabaseEntry::from_sbk(&entry?)?;
            if i < count {
                entry = change(entry, true);
                patch.upsert_entry(entry.clone())?;
            }
            if mode != "stream" {
                entries.push(entry);
            }
        }
        if mode == "stream" {
            book.write_sbk_with_patch(&mut writer, &patch)?;
            scanned = book.len();
        } else if mode == "full" {
            scanned = entries.len();
            BookDatabase::try_from_entries(entries)?
                .write_sbk(&mut writer, &SbkWriteOptions::new())?;
        } else {
            scanned = entries.len();
        }
    } else {
        let book = YaneuraOuBook::open_with_options(
            &args[3],
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
        )?;
        open_seconds = start.elapsed().as_secs_f64();
        for (i, entry) in book.iter_entries()?.enumerate() {
            if mode == "stream" && i >= count {
                break;
            }
            let mut entry = BookDatabaseEntry::from_yaneuraou(&entry?)?;
            if i < count {
                entry = change(entry, false);
                patch.upsert_entry(entry.clone())?;
            }
            if mode != "stream" {
                entries.push(entry);
            }
        }
        if mode == "stream" {
            book.write_yaneuraou_db2016_with_patch_with_control(&mut writer, &patch, |p| {
                scanned = p.processed_positions() as usize;
                BookControl::Continue
            })?;
        } else if mode == "full" {
            scanned = entries.len();
            BookDatabase::try_from_entries(entries)?.write_yaneuraou_db2016(
                &mut writer,
                &YaneuraOuDb2016WriteOptions::new().with_preserved_move_order(),
            )?;
        } else {
            scanned = entries.len();
        }
    }
    use std::io::Write;
    writer.flush()?;
    let encode_seconds = start.elapsed().as_secs_f64() - open_seconds;
    let sync_start = Instant::now();
    writer.get_ref().sync_all()?;
    println!(
        "{{\"open_seconds\":{open_seconds:.6},\"save_seconds\":{encode_seconds:.6},\"sync_seconds\":{:.6},\"positions\":{scanned}}}",
        sync_start.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(not(feature = "book"))]
fn main() {
    eprintln!("requires --features book");
}

#[cfg(feature = "book")]
fn fixtures(directory: &str) -> Result<(), Box<dyn std::error::Error>> {
    use rsshogi::types::Move;
    let directory = std::path::Path::new(directory);
    std::fs::create_dir_all(directory)?;
    for (label, initial, moves) in [
        ("chain", board::InitialPosition::Standard.to_sfen(), vec!["7g7f", "3c3d", "2g2f"]),
        ("cycle", "4k4/9/9/9/9/9/9/9/4K4 b - 1", vec!["5i4i", "5a4a", "4i5i", "4a5a"]),
    ] {
        let mut position = board::Position::from_sfen(initial)?;
        let mut entries = Vec::new();
        let mut positions = Vec::new();
        for usi in moves {
            positions.push(position.clone());
            let mv = Move::from_usi(usi).ok_or("invalid fixture move")?;
            entries.push(BookDatabaseEntry::new(
                BookPosition::from_position(&position, None),
                vec![BookCandidate::new(mv, None, None, None, None, None, BookMoveMetadata::new())],
                BookEntryMetadata::new(),
            ));
            position.apply_move32(position.move32_from_move(mv));
        }
        let path = directory.join(format!("{label}-source.sbk"));
        BookDatabase::try_from_entries(entries)?
            .write_sbk(&mut File::create(&path)?, &SbkWriteOptions::new())?;
        let book = SbkBook::open(&path)?;
        let mut patch = BookDatabasePatch::new();
        patch.remove_entry_by_sfen(&positions[0].to_sfen(None))?;
        book.write_sbk_with_patch(
            &mut File::create(directory.join(format!("{label}-delete-first.sbk")))?,
            &patch,
        )?;
        let mut patch = BookDatabasePatch::new();
        patch.remove_entry_by_sfen(&positions[1].to_sfen(None))?;
        book.write_sbk_with_patch(
            &mut File::create(directory.join(format!("{label}-delete-middle.sbk")))?,
            &patch,
        )?;
    }
    Ok(())
}
