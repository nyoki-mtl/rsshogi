//! 独立圧縮ブロックの索引と可変長符号化。

use super::crc::crc32c;
use super::wire::{
    BLOCK_INDEX_BYTES, MAX_BLOCK_BYTES, decode_candidate, encode_candidate, read_u64,
};
use super::{NativeBookEntry, NativeCandidate, invalid};
use crate::book::BookError;

#[derive(Clone, Debug)]
pub(super) struct BlockIndex {
    pub key: [u8; 32],
    pub offset: u64,
    pub compressed_bytes: u32,
    pub raw_bytes: u32,
    pub crc: u32,
}

impl BlockIndex {
    pub fn encode(&self) -> [u8; BLOCK_INDEX_BYTES] {
        let mut bytes = [0; BLOCK_INDEX_BYTES];
        bytes[..32].copy_from_slice(&self.key);
        bytes[32..40].copy_from_slice(&self.offset.to_le_bytes());
        bytes[40..44].copy_from_slice(&self.compressed_bytes.to_le_bytes());
        bytes[44..48].copy_from_slice(&self.raw_bytes.to_le_bytes());
        bytes[48..52].copy_from_slice(&self.crc.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, BookError> {
        if bytes.len() != BLOCK_INDEX_BYTES || bytes[52..].iter().any(|&b| b != 0) {
            return Err(invalid("invalid native block index"));
        }
        let row = Self {
            key: bytes[..32].try_into().expect("key bytes"),
            offset: read_u64(bytes, 32)?,
            compressed_bytes: u32::from_le_bytes(bytes[40..44].try_into().expect("four bytes")),
            raw_bytes: u32::from_le_bytes(bytes[44..48].try_into().expect("four bytes")),
            crc: u32::from_le_bytes(bytes[48..52].try_into().expect("four bytes")),
        };
        if row.compressed_bytes == 0
            || row.raw_bytes == 0
            || row.compressed_bytes as usize > MAX_BLOCK_BYTES
            || row.raw_bytes as usize > MAX_BLOCK_BYTES
        {
            return Err(invalid("native block exceeds size limit"));
        }
        Ok(row)
    }
}

fn put_varint(mut value: u32, output: &mut Vec<u8>) {
    while value >= 128 {
        output.push(value as u8 | 128);
        value >>= 7;
    }
    output.push(value as u8);
}

fn take<'a>(bytes: &mut &'a [u8], len: usize) -> Result<&'a [u8], BookError> {
    let part = bytes.get(..len).ok_or_else(|| invalid("truncated native block"))?;
    *bytes = &bytes[len..];
    Ok(part)
}

fn varint(bytes: &mut &[u8]) -> Result<u32, BookError> {
    let mut value = 0;
    for index in 0..5 {
        let byte = take(bytes, 1)?[0];
        if index == 4 && byte > 15 {
            return Err(invalid("native integer overflow"));
        }
        value |= u32::from(byte & 127) << (7 * index);
        if byte & 128 == 0 {
            if index != 0 && byte == 0 {
                return Err(invalid("noncanonical native integer"));
            }
            return Ok(value);
        }
    }
    Err(invalid("native integer overflow"))
}

fn encode_move(candidate: &NativeCandidate, bytes: &mut Vec<u8>) -> Result<(), BookError> {
    let fixed = encode_candidate(candidate)?;
    bytes.extend_from_slice(&fixed[..4]);
    if matches!(fixed[2], 1..=3) {
        let value = i32::from_le_bytes(fixed[4..8].try_into().expect("four bytes"));
        put_varint(((value as u32) << 1) ^ ((value >> 31) as u32), bytes);
    }
    bytes.push(u8::from(candidate.depth.is_some()));
    if let Some(depth) = candidate.depth {
        put_varint(depth, bytes);
    }
    put_varint(u32::from(candidate.display_order), bytes);
    Ok(())
}

fn decode_move(bytes: &mut &[u8]) -> Result<NativeCandidate, BookError> {
    let mut fixed = [0; 16];
    fixed[..4].copy_from_slice(take(bytes, 4)?);
    if matches!(fixed[2], 1..=3) {
        let value = varint(bytes)?;
        let signed = ((value >> 1) as i32) ^ -((value & 1) as i32);
        fixed[4..8].copy_from_slice(&signed.to_le_bytes());
    }
    let presence = take(bytes, 1)?[0];
    if presence > 1 {
        return Err(invalid("invalid native depth presence"));
    }
    fixed[12] = presence;
    if presence != 0 {
        fixed[8..12].copy_from_slice(&varint(bytes)?.to_le_bytes());
    }
    let order =
        u16::try_from(varint(bytes)?).map_err(|_| invalid("native display order overflow"))?;
    fixed[14..16].copy_from_slice(&order.to_le_bytes());
    decode_candidate(&fixed)
}

pub(super) fn encode(
    entries: &[NativeBookEntry],
    offset: u64,
) -> Result<(BlockIndex, Vec<u8>), BookError> {
    let mut bytes = Vec::new();
    let mut previous = [0; 32];
    for (index, entry) in entries.iter().enumerate() {
        let prefix = if index == 0 {
            0
        } else {
            entry.key.iter().zip(previous).take_while(|(a, b)| **a == *b).count()
        };
        bytes.push(prefix as u8);
        bytes.extend_from_slice(&entry.key[prefix..]);
        put_varint(entry.candidates.len() as u32, &mut bytes);
        for candidate in &entry.candidates {
            encode_move(candidate, &mut bytes)?;
        }
        if bytes.len() > MAX_BLOCK_BYTES {
            return Err(invalid("native block exceeds size limit"));
        }
        previous = entry.key;
    }
    let compressed = zstd::bulk::compress(&bytes, 3)?;
    if compressed.len() > MAX_BLOCK_BYTES {
        return Err(invalid("native block exceeds size limit"));
    }
    Ok((
        BlockIndex {
            key: entries[0].key,
            offset,
            compressed_bytes: compressed.len() as u32,
            raw_bytes: bytes.len() as u32,
            crc: crc32c(&compressed),
        },
        compressed,
    ))
}

pub(super) fn decode(
    row: &BlockIndex,
    compressed: &[u8],
    count: u64,
) -> Result<Vec<NativeBookEntry>, BookError> {
    if crc32c(compressed) != row.crc {
        return Err(invalid("native block checksum mismatch"));
    }
    if zstd::zstd_safe::find_frame_compressed_size(compressed)
        .map_err(|_| invalid("invalid native zstd frame"))?
        != compressed.len()
    {
        return Err(invalid("trailing native zstd data"));
    }
    let mut decoder = zstd::bulk::Decompressor::new()?;
    decoder.set_parameter(zstd::zstd_safe::DParameter::WindowLogMax(26))?;
    let raw = decoder.decompress(compressed, row.raw_bytes as usize)?;
    if raw.len() != row.raw_bytes as usize {
        return Err(invalid("native block expansion length mismatch"));
    }
    let mut bytes = raw.as_slice();
    let mut entries = Vec::with_capacity(count as usize);
    let mut previous = [0; 32];
    for index in 0..count {
        let prefix = usize::from(take(&mut bytes, 1)?[0]);
        if prefix > 31 || (index == 0 && prefix != 0) {
            return Err(invalid("invalid native key prefix"));
        }
        let mut key = previous;
        key[prefix..].copy_from_slice(take(&mut bytes, 32 - prefix)?);
        if (index == 0 && key != row.key) || (index != 0 && key <= previous) {
            return Err(invalid("unordered native block keys"));
        }
        if index != 0 && key.iter().zip(previous).take_while(|(a, b)| **a == *b).count() != prefix {
            return Err(invalid("noncanonical native key prefix"));
        }
        let moves = varint(&mut bytes)?;
        if moves > u32::from(u16::MAX) || moves as usize > bytes.len() / 6 {
            return Err(invalid("invalid native block candidate count"));
        }
        let mut candidates = Vec::with_capacity(moves as usize);
        for _ in 0..moves {
            candidates.push(decode_move(&mut bytes)?);
        }
        entries.push(NativeBookEntry { key, candidates });
        previous = key;
    }
    if !bytes.is_empty() {
        return Err(invalid("unreferenced native block bytes"));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_integer_boundaries_and_rejection() {
        for value in [0, 1, 127, 128, 16383, 16384, u32::MAX] {
            let mut bytes = Vec::new();
            put_varint(value, &mut bytes);
            let mut input = bytes.as_slice();
            assert_eq!(varint(&mut input).unwrap(), value);
            assert!(input.is_empty());
            for len in 0..bytes.len() {
                assert!(varint(&mut &bytes[..len]).is_err());
            }
        }
        for bytes in [&[128, 0][..], &[255, 255, 255, 255, 16][..], &[128, 128, 128, 128, 128][..]]
        {
            assert!(varint(&mut &bytes[..]).is_err());
        }
    }

    #[test]
    fn compressed_scores_depth_presence_and_corruption() {
        use crate::book::{NativeBound, NativeScore};
        use crate::types::Move;
        crate::board::init();
        let position = crate::board::hirate_position();
        for score in [
            NativeScore::Unknown,
            NativeScore::Cp { value: i32::MIN, bound: NativeBound::Exact },
            NativeScore::Cp { value: i32::MAX, bound: NativeBound::Lower },
            NativeScore::Mate { plies: -i32::MAX, bound: NativeBound::Upper },
            NativeScore::MateUnknown { winning: false },
            NativeScore::MateUnknown { winning: true },
        ] {
            for depth in [None, Some(0), Some(u32::MAX)] {
                let mut candidate = NativeCandidate::new(Move::from_usi("7g7f").unwrap());
                candidate.score = score;
                candidate.depth = depth;
                let entry = NativeBookEntry::from_position(&position, vec![candidate]).unwrap();
                let (row, bytes) = encode(std::slice::from_ref(&entry), 0).unwrap();
                assert_eq!(decode(&row, &bytes, 1).unwrap(), vec![entry]);
                let mut corrupt = bytes.clone();
                corrupt[0] ^= 1;
                assert!(decode(&row, &corrupt, 1).is_err());
                let mut wrong_size = row.clone();
                wrong_size.raw_bytes -= 1;
                assert!(decode(&wrong_size, &bytes, 1).is_err());
                let mut trailing = bytes;
                trailing.push(0);
                let mut row = row;
                row.crc = crc32c(&trailing);
                assert!(decode(&row, &trailing, 1).is_err());
            }
        }
    }
}
