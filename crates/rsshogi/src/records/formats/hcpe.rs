use crate::board::{HuffmanCodedPos, Position};
use crate::types::{AperyMove, GameResult, Move};
use std::fmt;
use std::io::{self, Read, Write};

pub const HCPE_ENTRY_BYTES: usize = 38;

#[repr(i8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HcpeGameResult {
    Draw = 0,
    BlackWin = 1,
    WhiteWin = 2,
}

impl TryFrom<i8> for HcpeGameResult {
    type Error = HcpeError;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Draw),
            1 => Ok(Self::BlackWin),
            2 => Ok(Self::WhiteWin),
            _ => Err(HcpeError::InvalidGameResult(value)),
        }
    }
}

impl TryFrom<GameResult> for HcpeGameResult {
    type Error = HcpeError;

    fn try_from(value: GameResult) -> Result<Self, Self::Error> {
        match value {
            GameResult::BlackWin => Ok(Self::BlackWin),
            GameResult::WhiteWin => Ok(Self::WhiteWin),
            GameResult::DrawByRepetition
            | GameResult::DrawByMaxPlies
            | GameResult::DrawByImpasse => Ok(Self::Draw),
            _ => Err(HcpeError::UnsupportedGameResult(value)),
        }
    }
}

impl fmt::Display for HcpeGameResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draw => write!(f, "Draw"),
            Self::BlackWin => write!(f, "BlackWin"),
            Self::WhiteWin => write!(f, "WhiteWin"),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HuffmanCodedPosAndEval {
    pub hcp: HuffmanCodedPos,
    pub eval: i16,
    pub best_move16: AperyMove,
    pub game_result: HcpeGameResult,
    pub dummy: u8,
}

impl HuffmanCodedPosAndEval {
    #[must_use]
    pub const fn new(
        hcp: HuffmanCodedPos,
        eval: i16,
        best_move16: AperyMove,
        game_result: HcpeGameResult,
    ) -> Self {
        Self { hcp, eval, best_move16, game_result, dummy: 0 }
    }

    #[must_use]
    pub fn from_position(
        position: &Position,
        eval: i16,
        best_move: Move,
        game_result: HcpeGameResult,
    ) -> Self {
        Self::new(position.to_huffman_coded_pos(), eval, best_move.to_apery(), game_result)
    }
}

#[derive(Debug)]
pub enum HcpeError {
    InvalidLength(usize),
    InvalidGameResult(i8),
    UnsupportedGameResult(GameResult),
    Io(io::Error),
}

impl fmt::Display for HcpeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength(len) => write!(f, "invalid hcpe length: {len}"),
            Self::InvalidGameResult(value) => write!(f, "invalid hcpe game result: {value}"),
            Self::UnsupportedGameResult(value) => {
                write!(f, "unsupported GameResult for hcpe export: {value:?}")
            }
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for HcpeError {}

impl From<io::Error> for HcpeError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[must_use]
pub fn encode_entry(entry: &HuffmanCodedPosAndEval) -> [u8; HCPE_ENTRY_BYTES] {
    let mut out = [0u8; HCPE_ENTRY_BYTES];
    out[..32].copy_from_slice(&entry.hcp.data);
    out[32..34].copy_from_slice(&entry.eval.to_le_bytes());
    out[34..36].copy_from_slice(&entry.best_move16.raw().to_le_bytes());
    out[36] = entry.game_result as i8 as u8;
    out[37] = entry.dummy;
    out
}

pub fn decode_entry(bytes: &[u8]) -> Result<HuffmanCodedPosAndEval, HcpeError> {
    if bytes.len() != HCPE_ENTRY_BYTES {
        return Err(HcpeError::InvalidLength(bytes.len()));
    }
    let mut hcp = [0u8; 32];
    hcp.copy_from_slice(&bytes[..32]);
    let eval = i16::from_le_bytes([bytes[32], bytes[33]]);
    let best_move16 = AperyMove::from_raw(u16::from_le_bytes([bytes[34], bytes[35]]));
    let game_result = HcpeGameResult::try_from(bytes[36] as i8)?;
    Ok(HuffmanCodedPosAndEval {
        hcp: HuffmanCodedPos { data: hcp },
        eval,
        best_move16,
        game_result,
        dummy: bytes[37],
    })
}

pub fn decode_entries<R: Read>(reader: &mut R) -> Result<Vec<HuffmanCodedPosAndEval>, HcpeError> {
    let mut entries = Vec::new();
    loop {
        let mut buf = [0u8; HCPE_ENTRY_BYTES];
        let mut read = 0usize;
        while read < HCPE_ENTRY_BYTES {
            match reader.read(&mut buf[read..])? {
                0 if read == 0 => return Ok(entries),
                0 => return Err(HcpeError::InvalidLength(read)),
                n => read += n,
            }
        }
        entries.push(decode_entry(&buf)?);
    }
}

pub fn write_entries<W: Write>(
    writer: &mut W,
    entries: &[HuffmanCodedPosAndEval],
) -> Result<(), HcpeError> {
    for entry in entries {
        writer.write_all(&encode_entry(entry))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::types::Move;
    use std::mem::size_of;

    #[test]
    fn test_hcpe_layout_and_roundtrip() {
        assert_eq!(size_of::<HuffmanCodedPosAndEval>(), HCPE_ENTRY_BYTES);

        board::init();
        let position = board::hirate_position();
        let entry = HuffmanCodedPosAndEval::from_position(
            &position,
            123,
            Move::from_usi("7g7f").unwrap(),
            HcpeGameResult::BlackWin,
        );
        let encoded = encode_entry(&entry);
        let decoded = decode_entry(&encoded).unwrap();
        assert_eq!(decoded, entry);
    }

    #[test]
    fn test_hcpe_io_helpers_roundtrip() {
        board::init();
        let position = board::hirate_position();
        let entries = vec![
            HuffmanCodedPosAndEval::from_position(
                &position,
                1,
                Move::from_usi("7g7f").unwrap(),
                HcpeGameResult::Draw,
            ),
            HuffmanCodedPosAndEval::from_position(
                &position,
                -2,
                Move::from_usi("2g2f").unwrap(),
                HcpeGameResult::WhiteWin,
            ),
        ];
        let mut bytes = Vec::new();
        write_entries(&mut bytes, &entries).unwrap();
        let decoded = decode_entries(&mut bytes.as_slice()).unwrap();
        assert_eq!(decoded, entries);
    }

    #[test]
    fn test_hcpe_game_result_rejects_unsupported_values() {
        assert!(HcpeGameResult::try_from(GameResult::BlackWinByDeclaration).is_err());
        let mut bytes = [0u8; HCPE_ENTRY_BYTES];
        bytes[36] = 9;
        assert!(decode_entry(&bytes).is_err());
    }
}
