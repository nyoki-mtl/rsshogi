use crate::book::BookError;
use crate::types::Move;

use super::crc::crc32c;
use super::types::{NativeBookMetadata, NativeBound, NativeCandidate, NativeScore, normalize_text};

pub(super) const HEADER_BYTES: usize = 128;
pub(super) const MOVE_BYTES: usize = 16;
pub(super) const BLOCK_INDEX_BYTES: usize = 56;
pub(super) const BLOCK_POSITIONS: u32 = 64;
pub(super) const MAX_BLOCK_POSITIONS: u32 = 16_384;
pub(super) const MAX_BLOCK_BYTES: usize = 64 * 1024 * 1024;
pub(super) const COMMENT_BYTES: usize = 32;
pub(super) const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_TX_BYTES: u64 = 256 * 1024 * 1024;

fn invalid(message: &'static str) -> BookError {
    BookError::InvalidFormat(message)
}

pub(super) fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, BookError> {
    let end = offset.checked_add(8).ok_or_else(|| invalid("native offset overflow"))?;
    let value = bytes.get(offset..end).ok_or_else(|| invalid("truncated native integer"))?;
    Ok(u64::from_le_bytes(value.try_into().expect("eight bytes")))
}

fn add_region(offset: u64, count: u64, size: u64) -> Result<u64, BookError> {
    count
        .checked_mul(size)
        .and_then(|bytes| offset.checked_add(bytes))
        .ok_or_else(|| invalid("native region overflow"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Header {
    pub node_count: u64,
    pub move_count: u64,
    pub comment_count: u64,
    pub block_index_offset: u64,
    pub blocks_offset: u64,
    pub blocks_bytes: u64,
    pub comment_index_offset: u64,
    pub comment_text_offset: u64,
    pub comment_text_bytes: u64,
    pub metadata_offset: u64,
    pub metadata_bytes: u64,
    pub base_end: u64,
    pub base_crc: u32,
    pub block_positions: u32,
    pub depth_kind: u32,
}

impl Header {
    pub(super) fn new(
        n: u64,
        m: u64,
        c: u64,
        t: u64,
        u: u64,
        compressed_bytes: u64,
        block_positions: u32,
    ) -> Result<Self, BookError> {
        if !(1..=MAX_BLOCK_POSITIONS).contains(&block_positions) {
            return Err(invalid("unsupported native block position count"));
        }
        if u != 0 && !(20..=20 + 2 * MAX_TEXT_BYTES as u64).contains(&u) {
            return Err(invalid("invalid native metadata size"));
        }
        let block_index_offset = HEADER_BYTES as u64;
        let blocks = n.div_ceil(u64::from(block_positions));
        let blocks_offset = add_region(block_index_offset, blocks, BLOCK_INDEX_BYTES as u64)?;
        let comment_index_offset = add_region(blocks_offset, compressed_bytes, 1)?;
        let comment_text_offset = add_region(comment_index_offset, c, COMMENT_BYTES as u64)?;
        let text_end = add_region(comment_text_offset, t, 1)?;
        let metadata_offset = if u == 0 { 0 } else { text_end };
        let base_end = add_region(text_end, u, 1)?;
        Ok(Self {
            node_count: n,
            move_count: m,
            comment_count: c,
            block_index_offset,
            blocks_offset,
            blocks_bytes: compressed_bytes,
            comment_index_offset,
            comment_text_offset,
            comment_text_bytes: t,
            metadata_offset,
            metadata_bytes: u,
            base_end,
            base_crc: 0,
            block_positions,
            depth_kind: 0,
        })
    }

    pub(super) fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0; HEADER_BYTES];
        bytes[..8].copy_from_slice(b"RSSHGBK4");
        bytes[8..10].copy_from_slice(&4u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&(HEADER_BYTES as u16).to_le_bytes());
        for (offset, value) in [
            (16, self.base_end),
            (24, self.node_count),
            (32, self.move_count),
            (40, self.comment_count),
            (48, self.block_index_offset),
            (56, self.blocks_offset),
            (64, self.blocks_bytes),
            (72, self.comment_index_offset),
            (80, self.comment_text_offset),
            (88, self.comment_text_bytes),
            (104, self.metadata_offset),
            (112, self.metadata_bytes),
        ] {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        bytes[96..100].copy_from_slice(&self.base_crc.to_le_bytes());
        bytes[120..124].copy_from_slice(&self.block_positions.to_le_bytes());
        bytes[124..128].copy_from_slice(&self.depth_kind.to_le_bytes());
        let checksum = crc32c(&bytes);
        bytes[100..104].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, BookError> {
        if bytes.len() != HEADER_BYTES {
            return Err(invalid("invalid native header size"));
        }
        if &bytes[..8] == b"RSHOGIBK" {
            return Err(BookError::Unsupported(
                "native v2 requires regeneration from source positions",
            ));
        }
        if &bytes[..8] != b"RSSHGBK4"
            || bytes[8..10] != 4u16.to_le_bytes()
            || bytes[10..12] != (HEADER_BYTES as u16).to_le_bytes()
        {
            return Err(invalid("unsupported native header"));
        }
        let expected_crc = u32::from_le_bytes(bytes[100..104].try_into().expect("four bytes"));
        let mut checked: [u8; HEADER_BYTES] = bytes.try_into().expect("header size");
        checked[100..104].fill(0);
        if crc32c(&checked) != expected_crc {
            return Err(invalid("native header checksum mismatch"));
        }
        if bytes[12..16].iter().any(|&value| value != 0) {
            return Err(BookError::Unsupported("native header flags or reserved fields"));
        }
        let mut header = Self::new(
            read_u64(bytes, 24)?,
            read_u64(bytes, 32)?,
            read_u64(bytes, 40)?,
            read_u64(bytes, 88)?,
            read_u64(bytes, 112)?,
            read_u64(bytes, 64)?,
            u32::from_le_bytes(bytes[120..124].try_into().expect("four bytes")),
        )?;
        for (offset, value) in [
            (16, header.base_end),
            (48, header.block_index_offset),
            (56, header.blocks_offset),
            (64, header.blocks_bytes),
            (72, header.comment_index_offset),
            (80, header.comment_text_offset),
            (104, header.metadata_offset),
        ] {
            if read_u64(bytes, offset)? != value {
                return Err(invalid("noncontiguous native regions"));
            }
        }
        header.depth_kind = u32::from_le_bytes(bytes[124..128].try_into().expect("four bytes"));
        header.base_crc = u32::from_le_bytes(bytes[96..100].try_into().expect("four bytes"));
        Ok(header)
    }
}

fn bound_byte(bound: NativeBound) -> u8 {
    match bound {
        NativeBound::Exact => 0,
        NativeBound::Lower => 1,
        NativeBound::Upper => 2,
    }
}

pub(super) fn encode_candidate(candidate: &NativeCandidate) -> Result<[u8; MOVE_BYTES], BookError> {
    candidate.validate()?;
    let mut bytes = [0; MOVE_BYTES];
    bytes[..2].copy_from_slice(&candidate.mv.raw().to_le_bytes());
    let (kind, bound, value) = match candidate.score {
        NativeScore::Unknown => (0, 0, 0),
        NativeScore::Cp { value, bound } => (1, bound_byte(bound), value),
        NativeScore::Mate { plies, bound } => (2, bound_byte(bound), plies),
        NativeScore::MateUnknown { winning } => (3, 0, if winning { 1 } else { -1 }),
    };
    bytes[2] = kind;
    bytes[3] = bound;
    bytes[4..8].copy_from_slice(&value.to_le_bytes());
    bytes[8..12].copy_from_slice(&candidate.depth.unwrap_or(0).to_le_bytes());
    let flags = u16::from(candidate.depth.is_some());
    bytes[12..14].copy_from_slice(&flags.to_le_bytes());
    bytes[14..16].copy_from_slice(&candidate.display_order.to_le_bytes());
    Ok(bytes)
}

pub(super) fn decode_candidate(bytes: &[u8]) -> Result<NativeCandidate, BookError> {
    if bytes.len() != MOVE_BYTES {
        return Err(invalid("invalid native candidate size"));
    }
    let bound = match bytes[3] {
        0 => NativeBound::Exact,
        1 => NativeBound::Lower,
        2 => NativeBound::Upper,
        _ => return Err(invalid("invalid native score bound")),
    };
    let value = i32::from_le_bytes(bytes[4..8].try_into().expect("four bytes"));
    let score = match bytes[2] {
        0 if value == 0 && bound == NativeBound::Exact => NativeScore::Unknown,
        1 => NativeScore::Cp { value, bound },
        2 if value != 0 && value != i32::MIN => NativeScore::Mate { plies: value, bound },
        3 if (value == 1 || value == -1) && bound == NativeBound::Exact => {
            NativeScore::MateUnknown { winning: value == 1 }
        }
        _ => return Err(invalid("invalid native score combination")),
    };
    let flags = u16::from_le_bytes(bytes[12..14].try_into().expect("two bytes"));
    if flags & !1 != 0 {
        return Err(BookError::Unsupported("native candidate presence flags"));
    }
    let depth = u32::from_le_bytes(bytes[8..12].try_into().expect("four bytes"));
    if flags & 1 == 0 && depth != 0 {
        return Err(invalid("nonzero absent native candidate field"));
    }
    let candidate = NativeCandidate {
        mv: Move::from_raw(u16::from_le_bytes(bytes[..2].try_into().expect("two bytes"))),
        score,
        depth: (flags & 1 != 0).then_some(depth),
        display_order: u16::from_le_bytes(bytes[14..16].try_into().expect("two bytes")),
    };
    candidate.validate()?;
    Ok(candidate)
}

pub(super) fn encode_metadata(metadata: &NativeBookMetadata) -> Result<Vec<u8>, BookError> {
    let metadata = metadata.normalized()?;
    let title = metadata.title.as_deref().unwrap_or("");
    let description = metadata.description.as_deref().unwrap_or("");
    let mut bytes = Vec::with_capacity(20 + title.len() + description.len());
    bytes.extend_from_slice(&(title.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(description.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&metadata.depth_kind.to_le_bytes());
    bytes.extend_from_slice(title.as_bytes());
    bytes.extend_from_slice(description.as_bytes());
    Ok(bytes)
}

pub(super) fn decode_metadata(bytes: &[u8]) -> Result<NativeBookMetadata, BookError> {
    let title_bytes = read_u64(bytes, 0)?;
    let description_bytes = read_u64(bytes, 8)?;
    if title_bytes > MAX_TEXT_BYTES as u64 || description_bytes > MAX_TEXT_BYTES as u64 {
        return Err(invalid("native metadata field exceeds 16 MiB"));
    }
    let title_end = 20 + title_bytes as usize;
    if bytes.len() != title_end + description_bytes as usize {
        return Err(invalid("invalid native metadata length"));
    }
    let decode_text = |text: &[u8]| {
        let text =
            std::str::from_utf8(text).map_err(|_| invalid("invalid native metadata utf-8"))?;
        if text.contains('\r') {
            return Err(invalid("noncanonical native metadata newline"));
        }
        normalize_text(Some(text.to_owned()))
    };
    Ok(NativeBookMetadata {
        depth_kind: u32::from_le_bytes(
            bytes
                .get(16..20)
                .ok_or_else(|| invalid("truncated metadata depth kind"))?
                .try_into()
                .expect("four bytes"),
        ),
        title: decode_text(&bytes[20..title_end])?,
        description: decode_text(&bytes[title_end..])?,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CommentIndex {
    pub position_id: u64,
    pub move_raw: u16,
    pub target_kind: u8,
    pub offset: u64,
    pub len: u64,
}

impl CommentIndex {
    pub(super) fn encode(&self) -> Result<[u8; COMMENT_BYTES], BookError> {
        self.validate()?;
        let mut bytes = [0; COMMENT_BYTES];
        bytes[..8].copy_from_slice(&self.position_id.to_le_bytes());
        bytes[8..10].copy_from_slice(&self.move_raw.to_le_bytes());
        bytes[10] = self.target_kind;
        bytes[16..24].copy_from_slice(&self.offset.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.len.to_le_bytes());
        Ok(bytes)
    }

    pub(super) fn decode(bytes: &[u8]) -> Result<Self, BookError> {
        if bytes.len() != COMMENT_BYTES {
            return Err(invalid("invalid native comment index size"));
        }
        if bytes[11..16].iter().any(|&value| value != 0) {
            return Err(BookError::Unsupported("native comment reserved fields"));
        }
        let index = Self {
            position_id: read_u64(bytes, 0)?,
            move_raw: u16::from_le_bytes(bytes[8..10].try_into().expect("two bytes")),
            target_kind: bytes[10],
            offset: read_u64(bytes, 16)?,
            len: read_u64(bytes, 24)?,
        };
        index.validate()?;
        Ok(index)
    }

    fn validate(&self) -> Result<(), BookError> {
        if self.len == 0
            || self.len > MAX_TEXT_BYTES as u64
            || self.offset.checked_add(self.len).is_none()
            || !matches!(self.target_kind, 0 | 1)
            || (self.target_kind == 0 && self.move_raw != 0)
            || (self.target_kind == 1 && !Move::from_raw(self.move_raw).is_normal())
        {
            return Err(invalid("invalid native comment index"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refresh_header_crc(bytes: &mut [u8; HEADER_BYTES]) {
        bytes[100..104].fill(0);
        let checksum = crc32c(bytes);
        bytes[100..104].copy_from_slice(&checksum.to_le_bytes());
    }

    #[test]
    fn header_layout_and_rejections() {
        let mut header = Header::new(0, 0, 0, 0, 0, 0, 64).unwrap();
        header.depth_kind = 99;
        assert_eq!(header.base_end, 128);
        assert_eq!(Header::decode(&header.encode()).unwrap(), header);
        for offset in [8, 12, 16, 48, 56, 64, 72, 80, 104] {
            let mut bytes = header.encode();
            bytes[offset] ^= 1;
            refresh_header_crc(&mut bytes);
            assert!(Header::decode(&bytes).is_err(), "offset {offset}");
        }
        for size in [0, 16_385, u32::MAX] {
            assert!(Header::new(0, 0, 0, 0, 0, 0, size).is_err());
        }
    }

    #[test]
    fn candidate_integer_boundaries() {
        let mut candidate = NativeCandidate::new(Move::from_usi("7g7f").unwrap());
        for score in [
            NativeScore::Unknown,
            NativeScore::Cp { value: i32::MIN, bound: NativeBound::Exact },
            NativeScore::Cp { value: i32::MAX, bound: NativeBound::Lower },
            NativeScore::Mate { plies: -i32::MAX, bound: NativeBound::Upper },
            NativeScore::MateUnknown { winning: true },
            NativeScore::MateUnknown { winning: false },
        ] {
            for depth in [None, Some(0), Some(u32::MAX)] {
                candidate.score = score;
                candidate.depth = depth;
                assert_eq!(
                    decode_candidate(&encode_candidate(&candidate).unwrap()).unwrap(),
                    candidate
                );
            }
        }
    }

    #[test]
    fn metadata_normalization_lengths_and_utf8() {
        let metadata = NativeBookMetadata {
            title: Some("a\r\nb\rc".into()),
            description: Some("説明".into()),
            depth_kind: u32::MAX,
        };
        let bytes = encode_metadata(&metadata).unwrap();
        assert_eq!(decode_metadata(&bytes).unwrap(), metadata.normalized().unwrap());
        for len in 0..bytes.len() {
            assert!(decode_metadata(&bytes[..len]).is_err());
        }
        let mut corrupt = bytes;
        corrupt[20] = 255;
        assert!(decode_metadata(&corrupt).is_err());
    }

    #[test]
    fn comment_index_known_bytes_and_rejections() {
        let index =
            CommentIndex { position_id: 256, move_raw: 0, target_kind: 0, offset: 7, len: 9 };
        let bytes = index.encode().unwrap();
        assert_eq!(&bytes[..16], &[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[16..24], &[7, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(CommentIndex::decode(&bytes).unwrap(), index);
        for (offset, value) in [(8, 1), (10, 2), (11, 1), (24, 0)] {
            let mut corrupt = bytes;
            corrupt[offset] = value;
            assert!(CommentIndex::decode(&corrupt).is_err());
        }
        assert!(read_u64(&bytes, usize::MAX).is_err());
        assert!(read_u64(&bytes, 25).is_err());
    }
}
