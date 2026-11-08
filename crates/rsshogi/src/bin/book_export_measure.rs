//! YBB/SBK 全件保存と差分保存の開発用計測。

#[cfg(feature = "book")]
mod measure {
    use rsshogi::{
        board::{self, Position},
        book::*,
    };
    use std::{
        env,
        fs::File,
        io::{self, BufWriter, Seek, SeekFrom, Write},
        time::{Duration, Instant},
    };

    struct Output {
        file: BufWriter<File>,
        io_time: Duration,
    }

    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let start = Instant::now();
            let result = self.file.write(bytes);
            self.io_time += start.elapsed();
            result
        }
        fn flush(&mut self) -> io::Result<()> {
            let start = Instant::now();
            let result = self.file.flush();
            self.io_time += start.elapsed();
            result
        }
    }

    impl Seek for Output {
        fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
            let start = Instant::now();
            let result = self.file.seek(from);
            self.io_time += start.elapsed();
            result
        }
    }

    fn database(input: &str, limit: usize, sbk: bool) -> Result<BookDatabase, BookError> {
        let book = YaneuraOuBook::open_with_options(
            input,
            YaneuraOuBookOpenOptions::with_access_mode(YaneuraOuAccessMode::AssumeSortedByCaller),
        )?;
        let entries = book
            .iter_entries()?
            .take(limit)
            .map(|source| {
                let source = source?;
                let candidates = source
                    .moves()
                    .iter()
                    .map(|mv| {
                        BookCandidate::new(
                            mv.mv(),
                            None,
                            mv.score(),
                            if sbk { None } else { mv.depth() },
                            None,
                            None,
                            if sbk {
                                BookMoveMetadata::from_sbk(SbkMoveMetadata::new(
                                    0,
                                    mv.score(),
                                    None,
                                    None,
                                ))
                            } else {
                                BookMoveMetadata::new()
                            },
                        )
                    })
                    .collect();
                Ok(BookDatabaseEntry::new(
                    BookPosition::from_sfen(source.sfen(), source.original_ply())?,
                    candidates,
                    BookEntryMetadata::new(),
                ))
            })
            .collect::<Result<Vec<_>, BookError>>()?;
        BookDatabase::try_from_entries(entries)
    }

    fn sbk_patch(book: &SbkBook, count: usize) -> Result<BookDatabasePatch, BookError> {
        let mut patch = BookDatabasePatch::new();
        for source in book.iter_entries().take(count) {
            let entry = BookDatabaseEntry::from_sbk(&source?)?;
            let metadata = entry.metadata().sbk().expect("SBK source metadata");
            let metadata = BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
                metadata.state_id(),
                metadata.state_index(),
                metadata.games(),
                metadata.won_black(),
                metadata.won_white(),
                Some(format!("{} [measure]", metadata.comment().unwrap_or_default())),
                metadata.evals().to_vec(),
            ));
            patch.upsert_entry(BookDatabaseEntry::new(
                entry.position().clone(),
                entry.candidates().to_vec(),
                metadata,
            ))?;
        }
        Ok(patch)
    }

    fn ybb_patch(book: &YbbBook, count: usize) -> Result<BookDatabasePatch, BookError> {
        let mut patch = BookDatabasePatch::new();
        for source in book.iter_entries().take(count) {
            let source = source?;
            let mut position = Position::empty();
            position
                .set_packed_sfen(&source.packed_sfen(), false, source.ply())
                .map_err(|error| BookError::InvalidData(error.to_string()))?;
            let candidates = source
                .moves()
                .iter()
                .map(|mv| {
                    BookCandidate::new(
                        mv.mv(),
                        None,
                        Some(i32::from(mv.eval().saturating_add(1))),
                        mv.depth().map(u32::from),
                        None,
                        None,
                        BookMoveMetadata::new(),
                    )
                })
                .collect();
            patch.upsert_entry(BookDatabaseEntry::new(
                BookPosition::from_position(&position, Some(u32::from(source.ply()))),
                candidates,
                BookEntryMetadata::new(),
            ))?;
        }
        Ok(patch)
    }

    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        board::init();
        let args: Vec<_> = env::args().collect();
        if args.len() != 7 {
            return Err("usage: book_export_measure <ybb|sbk> <full|patch> <input> <output> <limit> <changes>".into());
        }
        let sbk = args[1] == "sbk";
        let limit: usize = args[5].parse()?;
        let changes: usize = args[6].parse()?;
        let buffered = env::var("BOOK_MEASURE_UNBUFFERED").as_deref() != Ok("1");
        let mut output = Output {
            file: BufWriter::with_capacity(
                if buffered { 1024 * 1024 } else { 0 },
                File::create(&args[4])?,
            ),
            io_time: Duration::ZERO,
        };
        let prepare = Instant::now();
        let positions;
        let candidates;
        let prepare_seconds;
        let save_seconds;
        if args[2] == "full" {
            let db = database(&args[3], limit, sbk)?;
            positions = db.len();
            candidates = db.entries().iter().map(|entry| entry.candidates().len()).sum::<usize>();
            prepare_seconds = prepare.elapsed().as_secs_f64();
            let start = Instant::now();
            if sbk {
                db.write_sbk(&mut output, &SbkWriteOptions::new())?;
            } else {
                db.write_ybb(&mut output, &YbbWriteOptions::new().with_depth(true))?;
            }
            output.flush()?;
            save_seconds = start.elapsed().as_secs_f64();
        } else if sbk {
            let book = SbkBook::open(&args[3])?;
            let patch = sbk_patch(&book, changes)?;
            positions = book.len();
            candidates = 0;
            prepare_seconds = prepare.elapsed().as_secs_f64();
            let start = Instant::now();
            book.write_sbk_with_patch(&mut output, &patch)?;
            save_seconds = start.elapsed().as_secs_f64();
        } else {
            let book = YbbBook::open(&args[3])?;
            let patch = ybb_patch(&book, changes)?;
            positions = book.len() as usize;
            candidates = 0;
            prepare_seconds = prepare.elapsed().as_secs_f64();
            let start = Instant::now();
            book.write_ybb_with_patch(&mut output, &patch)?;
            save_seconds = start.elapsed().as_secs_f64();
        }
        let sync = Instant::now();
        output.file.get_ref().sync_all()?;
        println!(
            "{{\"format\":\"{}\",\"mode\":\"{}\",\"buffered\":{buffered},\"positions\":{positions},\"candidates\":{candidates},\"changes\":{changes},\"prepare_seconds\":{prepare_seconds},\"save_seconds\":{save_seconds},\"io_seconds\":{},\"sync_seconds\":{}}}",
            args[1],
            args[2],
            output.io_time.as_secs_f64(),
            sync.elapsed().as_secs_f64()
        );
        Ok(())
    }
}

#[cfg(feature = "book")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    measure::run()
}

#[cfg(not(feature = "book"))]
fn main() {
    eprintln!("requires --features book");
}
