//! 256ビット(`[u64; 4]`)バッファに対するビット単位の読み書きユーティリティ。
//!
//! `HuffmanCodedPos` と `PackedSfen` はどちらも同じビットレイアウトと同じバイト変換を
//! 共有するため、共通化してここに集約する。`BitReader` はカーソル溢れを
//! [`BitCursorOverflow`] で表し、各呼び出し側が自前のエラー型へ `From` 変換する。

/// ビットカーソルが256ビットのバッファ末尾を超えて読もうとしたことを表すエラー。
///
/// 各フォーマット固有のエラー型(`HuffmanCodedPosError` / `PackedSfenError`)へ
/// `From` 実装経由で変換され、`?` 演算子でそのまま伝播できる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BitCursorOverflow;

pub(super) struct BitWriter<'a> {
    data: &'a mut [u64; 4],
    cursor: u32,
}

impl<'a> BitWriter<'a> {
    pub(super) fn new(data: &'a mut [u64; 4]) -> Self {
        data.fill(0);
        Self { data, cursor: 0 }
    }

    pub(super) fn write_one_bit(&mut self, bit: bool) {
        if bit {
            let word_idx = (self.cursor >> 6) as usize;
            let offset = self.cursor & 63;
            self.data[word_idx] |= 1u64 << offset;
        }
        self.cursor += 1;
    }

    pub(super) fn write_n_bits(&mut self, value: u16, bits: u8) {
        let mut remaining = u32::from(bits);
        let mut data = u64::from(value);
        while remaining > 0 {
            let word_idx = (self.cursor >> 6) as usize;
            let offset = self.cursor & 63;
            let bits_in_word = 64 - offset;
            let write_bits = bits_in_word.min(remaining);
            let mask = if write_bits == 64 { u64::MAX } else { (1u64 << write_bits) - 1 };
            self.data[word_idx] |= (data & mask) << offset;
            self.cursor += write_bits;
            data >>= write_bits;
            remaining -= write_bits;
        }
    }

    pub(super) const fn cursor(&self) -> u32 {
        self.cursor
    }
}

pub(super) struct BitReader<'a> {
    data: &'a [u64; 4],
    cursor: u32,
}

impl<'a> BitReader<'a> {
    pub(super) const fn new(data: &'a [u64; 4]) -> Self {
        Self { data, cursor: 0 }
    }

    pub(super) fn read_one_bit(&mut self) -> Result<bool, BitCursorOverflow> {
        if self.cursor >= 256 {
            return Err(BitCursorOverflow);
        }
        let word_idx = (self.cursor >> 6) as usize;
        let offset = self.cursor & 63;
        let bit = (self.data[word_idx] >> offset) & 1;
        self.cursor += 1;
        Ok(bit != 0)
    }

    pub(super) fn read_n_bits(&mut self, bits: u8) -> Result<u16, BitCursorOverflow> {
        let mut value = 0u16;
        for i in 0..bits {
            if self.read_one_bit()? {
                value |= 1 << i;
            }
        }
        Ok(value)
    }

    /// カーソルを進めずに下位側から `bits` ビットを読む。
    ///
    /// 256 ビットの終端を越える部分は 0 として扱う。実際の消費時には
    /// [`Self::advance`] が境界を検証する。
    pub(super) fn peek_n_bits_zero_padded(&self, bits: u8) -> u16 {
        debug_assert!(bits <= 16, "peek supports at most 16 bits");
        if self.cursor >= 256 || bits == 0 {
            return 0;
        }

        let word_idx = (self.cursor >> 6) as usize;
        let offset = self.cursor & 63;
        let mut value = self.data[word_idx] >> offset;
        if offset + u32::from(bits) > 64 && word_idx + 1 < self.data.len() {
            value |= self.data[word_idx + 1] << (64 - offset);
        }
        let mask = (1u64 << bits) - 1;
        u16::try_from(value & mask).expect("at most 16 bits fit in u16")
    }

    /// カーソルを `bits` ビット進める。
    pub(super) fn advance(&mut self, bits: u8) -> Result<(), BitCursorOverflow> {
        let next = self.cursor.checked_add(u32::from(bits)).ok_or(BitCursorOverflow)?;
        if next > 256 {
            return Err(BitCursorOverflow);
        }
        self.cursor = next;
        Ok(())
    }

    pub(super) const fn cursor(&self) -> u32 {
        self.cursor
    }
}

pub(super) fn words_to_bytes(words: &[u64; 4]) -> [u8; 32] {
    let mut data = [0u8; 32];
    for (idx, word) in words.iter().enumerate() {
        let start = idx * 8;
        data[start..start + 8].copy_from_slice(&word.to_le_bytes());
    }
    data
}

pub(super) fn bytes_to_words(data: &[u8; 32]) -> [u64; 4] {
    let mut words = [0u64; 4];
    for (idx, word) in words.iter_mut().enumerate() {
        let start = idx * 8;
        let chunk: [u8; 8] = data[start..start + 8].try_into().expect("chunk fits");
        *word = u64::from_le_bytes(chunk);
    }
    words
}
