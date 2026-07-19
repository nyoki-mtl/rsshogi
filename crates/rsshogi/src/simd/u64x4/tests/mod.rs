use super::U64x4;

#[test]
fn test_u64x4_ops() {
    let a = U64x4::new(0xFF00, 0x0FF0, 0xAAAA, 0x5555);
    let b = U64x4::new(0x0F0F, 0x00FF, 0xFFFF, 0x0000);

    let and_res = a.and(b).parts();
    assert_eq!(and_res, [0x0F00, 0x00F0, 0xAAAA, 0x0000]);

    let or_res = a.or(b).parts();
    assert_eq!(or_res, [0xFF0F, 0x0FFF, 0xFFFF, 0x5555]);

    let xor_res = a.xor(b).parts();
    assert_eq!(xor_res, [0xF00F, 0x0F0F, 0x5555, 0x5555]);

    let and_not_res = a.and_not(b).parts();
    assert_eq!(and_not_res, [0xF000, 0x0F00, 0x0000, 0x5555]);
}

#[test]
fn test_u64x4_add_sub_shift() {
    let a = U64x4::new(1, 2, 3, 4);
    let b = U64x4::new(10, 20, 30, 40);
    assert_eq!(a.wrapping_add(b).parts(), [11, 22, 33, 44]);
    assert_eq!(b.wrapping_sub(a).parts(), [9, 18, 27, 36]);
    assert_eq!(a.shift_left(1).parts(), [2, 4, 6, 8]);
    assert_eq!(b.shift_right(1).parts(), [5, 10, 15, 20]);
}

#[test]
fn test_u64x4_byte_reverse_unpack_decrement_merge() {
    let value = U64x4::new(
        0x0123_4567_89ab_cdef,
        0x0011_2233_4455_6677,
        0x89ab_cdef_0123_4567,
        0xfedc_ba98_7654_3210,
    );
    let reversed = value.byte_reverse().parts();
    let expected = [
        0x7766_5544_3322_1100,
        0xefcd_ab89_6745_2301,
        0x1032_5476_98ba_dcfe,
        0x6745_2301_efcd_ab89,
    ];
    assert_eq!(reversed, expected);

    let hi = U64x4::new(1, 2, 3, 4);
    let lo = U64x4::new(5, 6, 7, 8);
    let (hi_out, lo_out) = U64x4::unpack(hi, lo);
    assert_eq!(hi_out.parts(), [6, 2, 8, 4]);
    assert_eq!(lo_out.parts(), [5, 1, 7, 3]);

    let (dec_hi, dec_lo) = U64x4::decrement_pair(hi, lo);
    assert_eq!(dec_lo.parts(), [4, 5, 6, 7]);
    assert_eq!(dec_hi.parts(), [1, 2, 3, 4]);

    let merged = U64x4::new(0x0F0F, 0x00FF, 0xF0F0, 0xFF00).merge();
    assert_eq!(merged, [0xFFFF, 0xFFFF]);
}
