#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BitCursorOverflow;

pub(super) struct BitWriter<'a> {
    data: &'a mut [u8; 32],
    cursor: u16,
}

impl<'a> BitWriter<'a> {
    pub(super) fn new(data: &'a mut [u8; 32]) -> Self {
        *data = [0; 32];
        Self { data, cursor: 0 }
    }

    pub(super) fn write_one_bit(&mut self, bit: bool) {
        debug_assert!(self.cursor < 256, "position codec buffer overflow");
        if self.cursor >= 256 {
            return;
        }
        if bit {
            self.data[usize::from(self.cursor / 8)] |= 1 << (self.cursor % 8);
        }
        self.cursor += 1;
    }

    pub(super) fn write_n_bits(&mut self, value: u16, bits: u8) {
        for shift in 0..bits {
            self.write_one_bit(((value >> shift) & 1) != 0);
        }
    }

    pub(super) const fn cursor(&self) -> u16 {
        self.cursor
    }
}

pub(super) struct BitReader<'a> {
    data: &'a [u8; 32],
    cursor: u16,
}

impl<'a> BitReader<'a> {
    pub(super) const fn new(data: &'a [u8; 32]) -> Self {
        Self { data, cursor: 0 }
    }

    pub(super) fn read_one_bit(&mut self) -> Result<bool, BitCursorOverflow> {
        if self.cursor >= 256 {
            return Err(BitCursorOverflow);
        }
        let bit = (self.data[usize::from(self.cursor / 8)] & (1 << (self.cursor % 8))) != 0;
        self.cursor += 1;
        Ok(bit)
    }

    pub(super) fn read_n_bits(&mut self, bits: u8) -> Result<u16, BitCursorOverflow> {
        let mut value = 0;
        for shift in 0..bits {
            if self.read_one_bit()? {
                value |= 1 << shift;
            }
        }
        Ok(value)
    }

    pub(super) const fn cursor(&self) -> u16 {
        self.cursor
    }
}
