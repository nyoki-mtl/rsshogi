//! SBK open の段階別時間を計測する開発用テスト。

use super::*;
use std::time::Instant;

struct Trace<R> {
    file: R,
    offsets: Option<Vec<u64>>,
}

impl<R: Read> Read for Trace<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(bytes)
    }
}

impl<R: Seek> Seek for Trace<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        if let SeekFrom::Start(offset) = position
            && let Some(offsets) = &mut self.offsets
        {
            offsets.push(offset);
        }
        self.file.seek(position)
    }
}

#[test]
#[ignore = "requires RSSHOGI_SBK_PROFILE_PATH"]
fn open_phase_measure() {
    crate::board::init();
    let path = std::path::PathBuf::from(
        std::env::var_os("RSSHOGI_SBK_PROFILE_PATH").expect("profile path"),
    );
    let capacity = std::env::var("RSSHOGI_SBK_BUFFER_KIB")
        .map_or(8, |v| v.parse::<usize>().expect("buffer KiB"))
        * 1024;
    let mut file = file::BufferedFile::with_capacity(File::open(&path).unwrap(), capacity).unwrap();
    let started = Instant::now();
    let scan = file::scan_file(&mut file).unwrap();
    let scan_seconds = started.elapsed().as_secs_f64();
    let mut book = SbkBook {
        path,
        states: scan.states,
        index: Vec::new(),
        state_lookup: Vec::new(),
        state_id_to_index: HashMap::new(),
        author: scan.author,
        description: scan.description,
        diagnostics: SbkDiagnostics::default(),
    };
    let id_started = Instant::now();
    book.state_lookup = vec![None; book.states.len()];
    book.build_state_id_index().unwrap();
    let id_seconds = id_started.elapsed().as_secs_f64();
    let graph_started = Instant::now();
    let trace_path = std::env::var_os("RSSHOGI_SBK_TRACE_PATH");
    trait ReadSeek: Read + Seek {}
    impl<T: Read + Seek> ReadSeek for T {}
    let io_mode = std::env::var("RSSHOGI_SBK_IO").unwrap_or_else(|_| "page".into());
    let mut source: Box<dyn ReadSeek> = match io_mode.as_str() {
        "page" => {
            let bytes = std::env::var("RSSHOGI_SBK_PAGE_BYTES")
                .map_or(page::PAGE_BYTES, |value| value.parse().expect("page bytes"));
            Box::new(page::PageFile::new(file.into_inner(), bytes, page::CACHE_BYTES).unwrap())
        }
        "buffered" => Box::new(file),
        _ => panic!("expected page or buffered I/O mode"),
    };
    let mut trace = Trace { file: &mut source, offsets: trace_path.as_ref().map(|_| Vec::new()) };
    book.build_index(&mut trace, &mut |_| BookControl::Continue).unwrap();
    let graph_seconds = graph_started.elapsed().as_secs_f64();
    if let Some(path) = trace_path {
        let mut out = std::io::BufWriter::new(File::create(path).unwrap());
        for offset in trace.offsets.unwrap() {
            out.write_all(&offset.to_le_bytes()).unwrap();
        }
    }
    println!(
        "buffer_bytes={capacity} scan_s={scan_seconds:.6} id_s={id_seconds:.6} graph_s={:.6} positions={} duplicates={} unresolved={}",
        graph_seconds,
        book.len(),
        book.diagnostics.duplicate_positions,
        book.diagnostics.unresolved_next_states
    );
}
