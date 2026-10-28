//! SBK のファイル上の wire フィールド走査。

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};

use super::{
    BookError, FIELD_SBOOK_AUTHOR, FIELD_SBOOK_DESCRIPTION, FIELD_SBOOK_STATE, FIELD_STATE_ID,
    SbkScan, SbkStateOffset,
};

pub(super) struct BufferedFile {
    reader: BufReader<File>,
    position: u64,
    len: u64,
}

impl BufferedFile {
    pub(super) fn new(file: File) -> io::Result<Self> {
        Self::with_capacity(file, 8 * 1024)
    }

    pub(super) fn with_capacity(mut file: File, capacity: usize) -> io::Result<Self> {
        let len = file.metadata()?.len();
        let position = file.stream_position()?;
        Ok(Self { reader: BufReader::with_capacity(capacity, file), position, len })
    }

    pub(super) const fn len(&self) -> u64 {
        self.len
    }

    pub(super) fn into_inner(self) -> File {
        self.reader.into_inner()
    }
}

impl Read for BufferedFile {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.reader.read(bytes)?;
        self.position += count as u64;
        Ok(count)
    }
}

impl Seek for BufferedFile {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let target = match position {
            SeekFrom::Start(target) => target,
            SeekFrom::Current(offset) => {
                self.position.checked_add_signed(offset).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "invalid seek position")
                })?
            }
            SeekFrom::End(offset) => self.len.checked_add_signed(offset).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid seek position")
            })?,
        };
        let distance = i128::from(target) - i128::from(self.position);
        if let Ok(distance) = i64::try_from(distance) {
            self.reader.seek_relative(distance)?;
        } else {
            self.reader.seek(SeekFrom::Start(target))?;
        }
        self.position = target;
        Ok(target)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.position)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileField {
    pub(super) field: u32,
    pub(super) wire_type: u32,
    pub(super) start: u64,
    pub(super) payload_offset: u64,
    pub(super) payload_bytes: u64,
    pub(super) end: u64,
}

fn read_varint(reader: &mut impl Read, remaining: &mut u64) -> Result<u64, BookError> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        if *remaining == 0 {
            return Err(BookError::InvalidFormat("truncated protobuf"));
        }
        let mut byte = [0];
        reader.read_exact(&mut byte)?;
        *remaining -= 1;
        value |= u64::from(byte[0] & 0x7f) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(BookError::InvalidFormat("protobuf varint is too long"))
}

pub(super) fn next_field<R: Read + Seek>(
    reader: &mut R,
    limit: u64,
) -> Result<Option<FileField>, BookError> {
    let start = reader.stream_position()?;
    if start >= limit {
        return Ok(None);
    }
    let mut remaining = limit - start;
    let tag = read_varint(reader, &mut remaining)?;
    let field = u32::try_from(tag >> 3).map_err(|_| BookError::InvalidFormat("field too large"))?;
    let wire_type = (tag & 0x07) as u32;
    if field == 0 {
        return Err(BookError::InvalidFormat("protobuf field number must be non-zero"));
    }
    let payload_bytes = match wire_type {
        0 => {
            let before = remaining;
            read_varint(reader, &mut remaining)?;
            before - remaining
        }
        1 => 8,
        2 => {
            let len = read_varint(reader, &mut remaining)?;
            usize::try_from(len)
                .map_err(|_| BookError::InvalidFormat("length-delimited field is too large"))?;
            len
        }
        5 => 4,
        _ => return Err(BookError::Unsupported("unsupported protobuf wire type")),
    };
    let cursor = limit - remaining;
    let payload_offset = if wire_type == 0 { cursor - payload_bytes } else { cursor };
    let end = payload_offset
        .checked_add(payload_bytes)
        .ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
    if end > limit {
        return Err(BookError::InvalidFormat("truncated length-delimited field"));
    }
    if end != cursor {
        if let Ok(distance) = i64::try_from(end - cursor) {
            reader.seek_relative(distance)?;
        } else {
            reader.seek(SeekFrom::Start(end))?;
        }
    }
    Ok(Some(FileField { field, wire_type, start, payload_offset, payload_bytes, end }))
}

pub(super) fn copy_span<R: Read + Seek, W: Write>(
    reader: &mut R,
    writer: &mut W,
    start: u64,
    end: u64,
) -> Result<(), BookError> {
    reader.seek(SeekFrom::Start(start))?;
    let len = end.checked_sub(start).ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
    if std::io::copy(&mut reader.take(len), writer)? != len {
        return Err(BookError::InvalidFormat("truncated length-delimited field"));
    }
    Ok(())
}

pub(super) fn scan_file<R: Read + Seek>(reader: &mut R) -> Result<SbkScan, BookError> {
    let limit = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let mut scan = SbkScan { author: None, description: None, states: Vec::new() };
    let mut payload = Vec::new();
    while let Some(field) = next_field(reader, limit)? {
        match (field.field, field.wire_type) {
            (FIELD_SBOOK_AUTHOR | FIELD_SBOOK_DESCRIPTION, 2) => {
                let len = usize::try_from(field.payload_bytes)
                    .map_err(|_| BookError::InvalidFormat("length-delimited field is too large"))?;
                reader.seek(SeekFrom::Start(field.payload_offset))?;
                payload.resize(len, 0);
                reader.read_exact(&mut payload)?;
                let text = String::from_utf8(payload.clone())
                    .map_err(|_| BookError::InvalidFormat("protobuf string is not UTF-8"))?;
                if field.field == FIELD_SBOOK_AUTHOR {
                    scan.author = Some(text);
                } else {
                    scan.description = Some(text);
                }
            }
            (FIELD_SBOOK_STATE, 2) => {
                reader.seek(SeekFrom::Start(field.payload_offset))?;
                let mut state_id = None;
                while let Some(state_field) = next_field(reader, field.end)? {
                    if (state_field.field, state_field.wire_type) == (FIELD_STATE_ID, 0) {
                        reader.seek(SeekFrom::Start(state_field.payload_offset))?;
                        let mut remaining = state_field.payload_bytes;
                        state_id = Some(read_varint(reader, &mut remaining)? as u32 as i32);
                        break;
                    }
                }
                scan.states.push(SbkStateOffset {
                    payload_offset: field.payload_offset,
                    payload_bytes: usize::try_from(field.payload_bytes).map_err(|_| {
                        BookError::InvalidFormat("length-delimited field is too large")
                    })?,
                    state_id,
                });
            }
            _ => {}
        }
        reader.seek(SeekFrom::Start(field.end))?;
    }
    Ok(scan)
}

#[cfg(test)]
mod tests;
