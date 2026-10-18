const fn make_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ (0x82f6_3b78 & (0u32.wrapping_sub(value & 1)));
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

const TABLE: [u32; 256] = make_table();

pub(super) struct Crc32c(u32);

impl Crc32c {
    pub(super) const fn new() -> Self {
        Self(u32::MAX)
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 >> 8) ^ TABLE[((self.0 ^ u32::from(byte)) & 255) as usize];
        }
    }

    pub(super) const fn finish(self) -> u32 {
        self.0 ^ u32::MAX
    }
}

pub(super) fn crc32c(bytes: &[u8]) -> u32 {
    let mut crc = Crc32c::new();
    crc.update(bytes);
    crc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bit_reference(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0x82f6_3b78 } else { crc >> 1 };
            }
        }
        !crc
    }

    #[test]
    fn known_vectors_and_independent_reference() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
        assert_eq!(crc32c(&[0; 32]), 0x8a91_36aa);
        let bytes: Vec<_> = (0..=255).cycle().take(8193).collect();
        for len in [1, 7, 32, 255, 256, 8193] {
            assert_eq!(crc32c(&bytes[..len]), bit_reference(&bytes[..len]));
            let mut crc = Crc32c::new();
            for chunk in bytes[..len].chunks(13) {
                crc.update(chunk);
            }
            assert_eq!(crc.finish(), bit_reference(&bytes[..len]));
        }
    }
}
