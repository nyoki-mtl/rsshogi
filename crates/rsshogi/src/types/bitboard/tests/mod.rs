use super::Bitboard;
use crate::types::{
    Rank, SQ_11, SQ_12, SQ_19, SQ_22, SQ_33, SQ_55, SQ_77, SQ_82, SQ_85, SQ_91, SQ_99, Square,
};

#[test]
fn test_bitboard_constants() {
    assert_eq!(Bitboard::EMPTY.packed_bits(), 0);
    assert_eq!(Bitboard::ALL.packed_bits(), (1u128 << 81) - 1);
    assert!(Bitboard::EMPTY.is_empty());
    assert!(!Bitboard::ALL.is_empty());
}

#[test]
fn test_bitboard_from_square() {
    let bb = Bitboard::from_square(SQ_11);
    assert_eq!(bb.packed_bits(), 1u128);
    assert!(bb.test(SQ_11));
    assert!(!bb.test(SQ_12));

    let bb = Bitboard::from_square(SQ_55);
    assert!(bb.test(SQ_55));
    assert_eq!(bb.count(), 1);
}

#[test]
fn test_bitboard_sq_occupied_and_pop_matches_yaneuraou() {
    for sq in 0..81 {
        let sq = Square::from_index(sq);
        let bb = Bitboard::from_square(sq);
        assert!(bb.test(sq));
        let mut bb = bb;
        assert_eq!(bb.pop_lsb(), Some(sq));
        assert!(bb.is_empty());
    }
}

#[test]
fn test_bitboard_set_clear() {
    let mut bb = Bitboard::EMPTY;

    bb.set(SQ_11);
    assert!(bb.test(SQ_11));
    assert_eq!(bb.count(), 1);

    bb.set(SQ_99);
    assert!(bb.test(SQ_99));
    assert_eq!(bb.count(), 2);

    bb.clear(SQ_11);
    assert!(!bb.test(SQ_11));
    assert!(bb.test(SQ_99));
    assert_eq!(bb.count(), 1);
}

#[test]
fn test_bitboard_test_index_matches_square_test() {
    let mut bb = Bitboard::EMPTY;
    for index in 0..Square::COUNT {
        if index % 3 == 0 || index == 62 || index == 63 || index == 80 {
            bb.set(Square::from_index(index));
        }
    }

    for index in 0..Square::COUNT {
        let sq = Square::from_index(index);
        assert_eq!(bb.test_index(index), bb.test(sq), "index {index}");
    }
}

#[test]
fn test_bitboard_pop_lsb() {
    let mut bb = Bitboard::EMPTY;
    bb.set(SQ_11);
    bb.set(SQ_55);
    bb.set(SQ_99);

    assert_eq!(bb.pop_lsb(), Some(SQ_11));
    assert_eq!(bb.count(), 2);

    assert_eq!(bb.pop_lsb(), Some(SQ_55));
    assert_eq!(bb.count(), 1);

    assert_eq!(bb.pop_lsb(), Some(SQ_99));
    assert_eq!(bb.count(), 0);

    assert_eq!(bb.pop_lsb(), None);
}

#[test]
fn test_bitboard_operations() {
    let mut bb1 = Bitboard::EMPTY;
    bb1.set(SQ_11);
    bb1.set(SQ_22);

    let mut bb2 = Bitboard::EMPTY;
    bb2.set(SQ_22);
    bb2.set(SQ_33);

    // AND
    let and = bb1.and(bb2);
    assert!(and.test(SQ_22));
    assert!(!and.test(SQ_11));
    assert!(!and.test(SQ_33));

    // OR
    let or = bb1.or(bb2);
    assert!(or.test(SQ_11));
    assert!(or.test(SQ_22));
    assert!(or.test(SQ_33));

    // XOR
    let xor = bb1.xor(bb2);
    assert!(xor.test(SQ_11));
    assert!(!xor.test(SQ_22));
    assert!(xor.test(SQ_33));

    // NOT
    let not = bb1.not();
    assert!(!not.test(SQ_11));
    assert!(!not.test(SQ_22));
    assert!(not.test(SQ_33));
}

#[test]
fn test_bitboard_iter() {
    let mut bb = Bitboard::EMPTY;
    bb.set(SQ_11);
    bb.set(SQ_55);
    bb.set(SQ_99);

    let squares: Vec<Square> = bb.iter().collect();
    assert_eq!(squares, vec![SQ_11, SQ_55, SQ_99]);
}

#[test]
fn test_bitboard_alignment() {
    assert_eq!(std::mem::align_of::<Bitboard>(), 16);
}

#[test]
fn test_bitboard_byte_reverse_matches_parts() {
    let mut bb = Bitboard::EMPTY;
    bb.set(SQ_11);
    bb.set(SQ_55);
    bb.set(SQ_85);
    let reversed = bb.byte_reverse();
    let [p0, p1] = bb.parts();
    let expected: [u64; 2] = (p1.swap_bytes(), p0.swap_bytes()).into();
    assert_eq!(reversed.parts(), expected);
}

#[test]
fn test_bitboard_byte_reverse_matches_yaneuraou() {
    let raw = (0x0123_4567_89ab_cdefu128) | ((0xfedc_ba98_7654_3210u128) << 64);
    let bb = Bitboard::from_raw_bits_unmasked(raw);
    let reversed = bb.byte_reverse();
    assert_eq!(reversed.parts(), [0x1032_5476_98ba_dcfe, 0xefcd_ab89_6745_2301]);
}

#[test]
fn test_bitboard_rank9_mask_matches_yaneuraou() {
    let raw = (0x3fdf_eff7_fbfd_feffu128) | ((0x0000_0000_0001_feffu128) << 64);
    let mask = Bitboard::from_raw_bits_unmasked(raw);
    let rank9 = mask.not();
    assert_eq!(rank9, Bitboard::rank_mask(Rank::RANK_9));
}

#[test]
fn test_bitboard_flip_mirror_file() {
    let mut bb = Bitboard::EMPTY;
    bb.set(SQ_11);
    bb.set(SQ_55);
    bb.set(SQ_99);

    let flipped = bb.flip();
    assert!(flipped.test(SQ_99));
    assert!(flipped.test(SQ_55));
    assert!(flipped.test(SQ_11));

    let mirrored = bb.mirror_file();
    assert!(mirrored.test(SQ_91));
    assert!(mirrored.test(SQ_55));
    assert!(mirrored.test(SQ_19));
}

#[test]
fn test_bitboard_unpack_and_decrement_pair() {
    let hi = Bitboard::from_packed_bits(0x11u128);
    let lo = Bitboard::from_packed_bits(0x22u128);
    let (hi_out, lo_out) = Bitboard::unpack(hi, lo);
    let [hi_p0, hi_p1] = hi.parts();
    let [lo_p0, lo_p1] = lo.parts();
    assert_eq!(hi_out.parts(), [lo_p1, hi_p1]);
    assert_eq!(lo_out.parts(), [lo_p0, hi_p0]);

    let hi = Bitboard::from_parts([0x1111, 0x2222]);
    let lo = Bitboard::from_parts([0x3333, 0x4444]);
    let (hi_out, lo_out) = Bitboard::unpack(hi, lo);
    assert_eq!(hi_out.parts(), [0x4444, 0x2222]);
    assert_eq!(lo_out.parts(), [0x3333, 0x1111]);

    let (dec_hi, dec_lo) = Bitboard::decrement_pair(hi, lo);
    let [dec_hi_p0, dec_hi_p1] = dec_hi.parts();
    let [dec_lo_p0, dec_lo_p1] = dec_lo.parts();
    let [hi_p0, hi_p1] = hi.parts();
    let [lo_p0, lo_p1] = lo.parts();
    let expected_hi_p0 = hi_p0.wrapping_sub(u64::from(lo_p0 == 0));
    let expected_hi_p1 = hi_p1.wrapping_sub(u64::from(lo_p1 == 0));
    assert_eq!(dec_hi_p0, expected_hi_p0);
    assert_eq!(dec_hi_p1, expected_hi_p1);
    assert_eq!(dec_lo_p0, lo_p0.wrapping_sub(1));
    assert_eq!(dec_lo_p1, lo_p1.wrapping_sub(1));
}

/// Bitboard 演算を成分ごとの演算と照合する。
#[test]
#[allow(clippy::cognitive_complexity)]
fn test_bitboard_operations_efficiency() {
    // 複数ビット設定
    let mut bb1 = Bitboard::EMPTY;
    for sq in [SQ_11, SQ_22, SQ_33, SQ_55, SQ_77, SQ_99] {
        bb1.set(sq);
    }

    let mut bb2 = Bitboard::EMPTY;
    for sq in [SQ_22, SQ_55, SQ_82, SQ_99] {
        bb2.set(sq);
    }

    // AND演算（交差部分）
    let and_result = bb1 & bb2;
    assert_eq!(and_result.count(), 3); // SQ_22, SQ_55, SQ_99
    assert!(and_result.test(SQ_22));
    assert!(and_result.test(SQ_55));
    assert!(and_result.test(SQ_99));

    // OR演算（和集合）
    let or_result = bb1 | bb2;
    assert_eq!(or_result.count(), 7); // 全ビット
    assert!(or_result.test(SQ_11));
    assert!(or_result.test(SQ_82));

    // XOR演算（対称差）
    let xor_result = bb1 ^ bb2;
    assert_eq!(xor_result.count(), 4); // SQ_11, SQ_33, SQ_77, SQ_82
    assert!(xor_result.test(SQ_11));
    assert!(!xor_result.test(SQ_22)); // 共通ビットは0

    // NOT演算（補集合）
    let not_result = !bb1;
    assert_eq!(not_result.count(), 81 - 6);
    assert!(!not_result.test(SQ_11));
    assert!(not_result.test(SQ_12));

    // popcount（ビット数カウント）
    assert_eq!(bb1.count(), 6);
    assert_eq!(bb2.count(), 4);

    // LSB（最下位ビット）
    assert_eq!(bb1.lsb(), Some(SQ_11));
    assert_eq!(bb2.lsb(), Some(SQ_22));

    // MSB（最上位ビット）
    assert_eq!(bb1.msb(), Some(SQ_99));
    assert_eq!(bb2.msb(), Some(SQ_99));

    // 空ビットボードでの動作
    let empty = Bitboard::EMPTY;
    assert_eq!(empty.count(), 0);
    assert_eq!(empty.lsb(), None);
    assert_eq!(empty.msb(), None);

    // 全ビット設定でのNOT
    let all = Bitboard::ALL;
    assert_eq!(all.count(), 81);
    let not_all = !all;
    assert_eq!(not_all.count(), 0);
}
