//! Native Book v4 用の CRC-32C（Castagnoli）計算。
//!
//! 対応する CPU 命令（AArch64 の `crc` 拡張、x86_64 の SSE4.2）が利用できる場合は
//! ハードウェア命令で 8 バイト単位に更新し、それ以外の環境や Miri では 256 要素の
//! ルックアップテーブルにフォールバックする。

#[cfg(any(
    test,
    miri,
    not(any(
        all(target_arch = "aarch64", target_feature = "crc"),
        all(target_arch = "x86_64", target_feature = "sse4.2"),
    ))
))]
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

#[cfg(any(
    test,
    miri,
    not(any(
        all(target_arch = "aarch64", target_feature = "crc"),
        all(target_arch = "x86_64", target_feature = "sse4.2"),
    ))
))]
const TABLE: [u32; 256] = make_table();

#[cfg(any(
    test,
    miri,
    not(any(
        all(target_arch = "aarch64", target_feature = "crc"),
        all(target_arch = "x86_64", target_feature = "sse4.2"),
    ))
))]
fn update_table(mut crc: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        crc = (crc >> 8) ^ TABLE[((crc ^ u32::from(byte)) & 255) as usize];
    }
    crc
}

#[cfg(all(not(miri), target_arch = "aarch64"))]
#[target_feature(enable = "crc")]
fn update_aarch64_crc(mut crc: u32, bytes: &[u8]) -> u32 {
    use core::arch::aarch64::{__crc32cb, __crc32cd};

    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("chunks_exact(8) yields 8 bytes"));
        crc = __crc32cd(crc, word);
    }
    for &byte in chunks.remainder() {
        crc = __crc32cb(crc, byte);
    }
    crc
}

#[cfg(all(not(miri), target_arch = "x86_64"))]
#[target_feature(enable = "sse4.2")]
fn update_x86_64_sse42(mut crc: u32, bytes: &[u8]) -> u32 {
    use core::arch::x86_64::{_mm_crc32_u8, _mm_crc32_u64};

    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let word = u64::from_le_bytes(chunk.try_into().expect("chunks_exact(8) yields 8 bytes"));
        crc = _mm_crc32_u64(u64::from(crc), word) as u32;
    }
    for &byte in chunks.remainder() {
        crc = _mm_crc32_u8(crc, byte);
    }
    crc
}

fn dispatch_update(crc: u32, bytes: &[u8]) -> u32 {
    #[cfg(all(not(miri), target_arch = "aarch64", target_feature = "crc"))]
    {
        // SAFETY: コンパイル対象の `target_feature = "crc"` が有効である。
        unsafe { update_aarch64_crc(crc, bytes) }
    }
    #[cfg(all(not(miri), target_arch = "aarch64", not(target_feature = "crc")))]
    {
        if std::arch::is_aarch64_feature_detected!("crc") {
            // SAFETY: 実行時に `crc` 拡張の有効性を確認済みである。
            unsafe { update_aarch64_crc(crc, bytes) }
        } else {
            update_table(crc, bytes)
        }
    }
    #[cfg(all(not(miri), target_arch = "x86_64", target_feature = "sse4.2"))]
    {
        // SAFETY: コンパイル対象の `target_feature = "sse4.2"` が有効である。
        unsafe { update_x86_64_sse42(crc, bytes) }
    }
    #[cfg(all(not(miri), target_arch = "x86_64", not(target_feature = "sse4.2")))]
    {
        if std::arch::is_x86_feature_detected!("sse4.2") {
            // SAFETY: 実行時に SSE4.2 命令の有効性を確認済みである。
            unsafe { update_x86_64_sse42(crc, bytes) }
        } else {
            update_table(crc, bytes)
        }
    }
    #[cfg(any(miri, not(any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        update_table(crc, bytes)
    }
}

pub(super) struct Crc32c(u32);

impl Crc32c {
    pub(super) const fn new() -> Self {
        Self(u32::MAX)
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        self.0 = dispatch_update(self.0, bytes);
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

    fn table_reference(bytes: &[u8]) -> u32 {
        update_table(u32::MAX, bytes) ^ u32::MAX
    }

    #[test]
    fn known_vectors_and_independent_reference() {
        assert_eq!(crc32c(b"123456789"), 0xe306_9283);
        assert_eq!(crc32c(b""), 0);
        assert_eq!(crc32c(&[0; 32]), 0x8a91_36aa);
        assert_eq!(table_reference(b"123456789"), 0xe306_9283);

        let bytes: Vec<_> = (0u8..=255).cycle().take(8209).collect();
        for offset in 0..16 {
            for len in [0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 63, 64, 255, 256, 8193] {
                let slice = &bytes[offset..offset + len];
                let expected = bit_reference(slice);
                assert_eq!(table_reference(slice), expected);
                assert_eq!(crc32c(slice), expected);

                for step in [1, 3, 8, 13, 64] {
                    let mut crc = Crc32c::new();
                    for chunk in slice.chunks(step) {
                        crc.update(chunk);
                    }
                    assert_eq!(crc.finish(), expected);
                }
            }
        }
    }
}
