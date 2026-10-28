use super::*;
use std::fs::File;
use std::io::Write;
use std::{cell::Cell, io::Cursor, rc::Rc};

struct Source {
    bytes: Cursor<Vec<u8>>,
    reads: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}

impl Read for Source {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        if self.fail.replace(false) {
            // 失敗前にスロットの一部が上書きされた場合も検査する。
            if !output.is_empty() {
                output[0] = 255;
            }
            return Err(io::Error::other("injected read failure"));
        }
        self.bytes.read(output)
    }
}
impl Seek for Source {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.bytes.seek(position)
    }
}

#[test]
fn caches_separate_pages_and_invalidates_failed_replacements() {
    let reads = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    let source =
        Source { bytes: Cursor::new((0..31).collect()), reads: reads.clone(), fail: fail.clone() };
    let mut file = PageFile::new(source, 4, 8).unwrap();
    let mut byte = [0];
    for offset in [0, 4, 1, 5] {
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.read_exact(&mut byte).unwrap();
        assert_eq!(byte[0], offset as u8);
    }
    assert_eq!(reads.get(), 2);
    file.seek(SeekFrom::Start(8)).unwrap();
    fail.set(true);
    assert!(file.read(&mut byte).is_err());
    assert_eq!(file.stream_position().unwrap(), 8);
    file.rewind().unwrap();
    file.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [0]);
    assert_eq!(reads.get(), 4);
}

#[test]
fn mixed_reads_and_seeks_match_cursor_including_boundaries_and_eof() {
    let bytes: Vec<_> = (0..10003).map(|i| (i % 251) as u8).collect();
    let mut file = PageFile::new(Cursor::new(bytes.clone()), 128, 512).unwrap();
    let mut expected = Cursor::new(bytes);
    let mut seed = 17u64;
    for _ in 0..2000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = (seed >> 32) % 10100;
        assert_eq!(
            file.seek(SeekFrom::Start(offset)).unwrap(),
            expected.seek(SeekFrom::Start(offset)).unwrap()
        );
        let count = (seed % 401) as usize;
        let mut actual = vec![0; count];
        let mut reference = vec![0; count];
        let a = file.read_exact(&mut actual);
        // Cursor::read_exact は EOF で位置を末尾へ詰めるため、File と同じ read ループを使う。
        let b = (|| -> io::Result<()> {
            let mut rest = reference.as_mut_slice();
            while !rest.is_empty() {
                let count = expected.read(rest)?;
                if count == 0 {
                    return Err(io::ErrorKind::UnexpectedEof.into());
                }
                rest = &mut rest[count..];
            }
            Ok(())
        })();
        assert_eq!(a.as_ref().err().map(io::Error::kind), b.as_ref().err().map(io::Error::kind));
        assert_eq!(actual, reference);
        assert_eq!(file.stream_position().unwrap(), expected.position());
    }
    assert_eq!(file.seek(SeekFrom::End(-3)).unwrap(), 10000);
    let mut end = Vec::new();
    file.read_to_end(&mut end).unwrap();
    assert_eq!(end, [211, 212, 213]);
    let old = file.stream_position().unwrap();
    assert!(file.seek(SeekFrom::Current(-20000)).is_err());
    assert_eq!(file.stream_position().unwrap(), old);
    let mut empty = PageFile::new(Cursor::new(Vec::<u8>::new()), 128, 512).unwrap();
    assert_eq!(empty.read(&mut [0; 1]).unwrap(), 0);
}

#[test]
fn cache_reads_real_file_boundaries() {
    let serial =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("sbk-positioned-{}-{serial}", std::process::id()));
    let bytes: Vec<_> = (0..10003).map(|i| (i % 251) as u8).collect();
    let mut disk = File::options().read(true).write(true).create_new(true).open(&path).unwrap();
    disk.write_all(&bytes).unwrap();
    let mut file = PageFile::new(disk, 512, 4096).unwrap();
    for offset in [0, 511, 512, 9990, 12, 8191, 1] {
        file.seek(SeekFrom::Start(offset)).unwrap();
        let mut actual = [0; 10];
        file.read_exact(&mut actual).unwrap();
        assert_eq!(actual, bytes[offset as usize..offset as usize + 10]);
    }
    file.seek(SeekFrom::End(-10)).unwrap();
    let mut actual = Vec::new();
    file.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, bytes[9993..]);
    file.seek(SeekFrom::Start(11000)).unwrap();
    assert_eq!(file.read(&mut [0; 1]).unwrap(), 0);
    assert_eq!(file.stream_position().unwrap(), 11000);
    drop(file);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn partial_and_interrupted_page_fills_are_completed() {
    struct Partial {
        bytes: Cursor<Vec<u8>>,
        interrupt: bool,
    }
    impl Read for Partial {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            if self.interrupt {
                self.interrupt = false;
                return Err(io::ErrorKind::Interrupted.into());
            }
            let len = out.len().min(3);
            self.bytes.read(&mut out[..len])
        }
    }
    impl Seek for Partial {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.bytes.seek(pos)
        }
    }
    let expected: Vec<_> = (0..35).collect();
    let mut file =
        PageFile::new(Partial { bytes: Cursor::new(expected.clone()), interrupt: true }, 16, 32)
            .unwrap();
    let mut first = [0; 5];
    file.read_exact(&mut first).unwrap();
    assert_eq!(first, [0, 1, 2, 3, 4]);
    file.rewind().unwrap();
    let mut actual = Vec::new();
    file.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, expected);
}
