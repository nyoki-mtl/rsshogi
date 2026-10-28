//! グラフ順の読み込みに使う固定上限のページキャッシュ。

use std::io::{self, Read, Seek, SeekFrom};

pub(super) const PAGE_BYTES: usize = 512;
pub(super) const CACHE_BYTES: usize = 4 * 1024 * 1024;

fn seek_position(position: u64, len: u64, seek: SeekFrom) -> io::Result<u64> {
    match seek {
        SeekFrom::Start(target) => Some(target),
        SeekFrom::Current(offset) => position.checked_add_signed(offset),
        SeekFrom::End(offset) => len.checked_add_signed(offset),
    }
    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid seek position"))
}

pub(super) struct PageFile<R> {
    reader: R,
    position: u64,
    len: u64,
    page_bytes: usize,
    tags: Vec<Option<u64>>,
    bytes: Vec<u8>,
}

impl<R: Read + Seek> PageFile<R> {
    pub(super) fn new(mut reader: R, page_bytes: usize, budget: usize) -> io::Result<Self> {
        assert!(page_bytes > 0 && budget >= page_bytes);
        let position = reader.stream_position()?;
        let len = reader.seek(SeekFrom::End(0))?;
        let pages =
            len.div_ceil(page_bytes as u64).max(1).min((budget / page_bytes) as u64) as usize;
        Ok(Self {
            reader,
            position,
            len,
            page_bytes,
            tags: vec![None; pages],
            bytes: vec![0; pages * page_bytes],
        })
    }
}

impl<R: Read + Seek> Read for PageFile<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.len || output.is_empty() {
            return Ok(0);
        }
        // 大きい state はキャッシュに分割せず、呼び出し側の再利用バッファへ読む。
        if output.len() >= self.page_bytes {
            self.reader.seek(SeekFrom::Start(self.position))?;
            let count = self.reader.read(output)?;
            self.position += count as u64;
            return Ok(count);
        }
        let page = self.position / self.page_bytes as u64;
        let slot = (page % self.tags.len() as u64) as usize;
        let start = slot * self.page_bytes;
        let page_offset = page * self.page_bytes as u64;
        if self.tags[slot] != Some(page) {
            self.tags[slot] = None;
            self.reader.seek(SeekFrom::Start(page_offset))?;
            let count = (self.len - page_offset).min(self.page_bytes as u64) as usize;
            self.reader.read_exact(&mut self.bytes[start..start + count])?;
            self.tags[slot] = Some(page);
        }
        let within = (self.position - page_offset) as usize;
        let count = output
            .len()
            .min(self.page_bytes - within)
            .min((self.len - self.position).min(usize::MAX as u64) as usize);
        output[..count].copy_from_slice(&self.bytes[start + within..start + within + count]);
        self.position += count as u64;
        Ok(count)
    }
}

impl<R: Read + Seek> Seek for PageFile<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.position = seek_position(self.position, self.len, position)?;
        Ok(self.position)
    }
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests;
