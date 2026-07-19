#![allow(clippy::missing_const_for_fn)]

// Reckless方式に合わせて、ビルド時のtarget_featureでSIMD実装を選択する。
// AVX512/NEONは未導入のため、AVX2が有効ならAVX2、それ以外はScalarを使う。

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
mod avx2;
#[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
mod scalar;

#[cfg(all(target_arch = "x86_64", target_feature = "avx2"))]
pub use avx2::*;
#[cfg(not(all(target_arch = "x86_64", target_feature = "avx2")))]
pub use scalar::*;

#[cfg(test)]
mod tests;
