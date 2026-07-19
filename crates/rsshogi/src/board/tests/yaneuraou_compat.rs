use crate::board::test_support::move_from_usi_expect;
use crate::board::zobrist::ZobristKey;
use crate::board::{Bitboard, Position};
use crate::types::{Color, Piece, PieceType, Square};

const ZOBRIST_SEED: u64 = 20_151_225;
const PRNG_MULTIPLIER: u64 = 2_685_821_657_736_338_717;

struct Prng {
    state: u64,
}

impl Prng {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        let mut s = self.state;
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        self.state = s;
        s.wrapping_mul(PRNG_MULTIPLIER)
    }
}

fn next_key(rng: &mut Prng) -> ZobristKey {
    let low = rng.next_u64();
    #[cfg(feature = "hash-128")]
    {
        let high = rng.next_u64();
        let _ = rng.next_u64();
        let _ = rng.next_u64();
        ZobristKey::new(low, high)
    }
    #[cfg(not(feature = "hash-128"))]
    {
        let _ = rng.next_u64();
        let _ = rng.next_u64();
        let _ = rng.next_u64();
        ZobristKey::new(low, 0)
    }
}

fn between_expected(from: Square, to: Square) -> Bitboard {
    let from_file = from.raw() / 9;
    let from_rank = from.raw() % 9;
    let to_file = to.raw() / 9;
    let to_rank = to.raw() % 9;

    let df = to_file - from_file;
    let dr = to_rank - from_rank;
    if df == 0 && dr == 0 {
        return Bitboard::EMPTY;
    }
    if df != 0 && dr != 0 && df.abs() != dr.abs() {
        return Bitboard::EMPTY;
    }

    let steps = df.abs().max(dr.abs());
    if steps <= 1 {
        return Bitboard::EMPTY;
    }

    let file_step = df.signum();
    let rank_step = dr.signum();
    let mut result = Bitboard::EMPTY;
    for i in 1..steps {
        let file = from_file + file_step * i;
        let rank = from_rank + rank_step * i;
        let idx = file * 9 + rank;
        result.set(Square::new(idx));
    }
    result
}

fn line_direction_index(from: Square, to: Square) -> Option<usize> {
    if from == to {
        return Some(0);
    }
    let from_file = from.raw() / 9;
    let from_rank = from.raw() % 9;
    let to_file = to.raw() / 9;
    let to_rank = to.raw() % 9;

    let df = to_file - from_file;
    let dr = to_rank - from_rank;

    if df == 0 {
        return Some(3);
    }
    if dr == 0 {
        return Some(1);
    }
    if df == dr || df == -dr {
        let same_sign = (df > 0 && dr > 0) || (df < 0 && dr < 0);
        return Some(if same_sign { 0 } else { 2 });
    }
    None
}

fn line_from_delta(sq: Square, file_step: i8, rank_step: i8) -> Bitboard {
    let mut line = Bitboard::EMPTY;
    line.set(sq);

    for sign in [-1, 1] {
        let mut file = sq.raw() / 9;
        let mut rank = sq.raw() % 9;
        loop {
            file += file_step * sign;
            rank += rank_step * sign;
            if !(0..9).contains(&file) || !(0..9).contains(&rank) {
                break;
            }
            let idx = file * 9 + rank;
            line.set(Square::new(idx));
        }
    }

    line
}

fn line_expected(from: Square, to: Square) -> Bitboard {
    if from == to {
        return Bitboard::from_square(from);
    }
    let Some(dir) = line_direction_index(from, to) else {
        return Bitboard::EMPTY;
    };
    match dir {
        0 => line_from_delta(from, -1, -1),
        1 => line_from_delta(from, -1, 0),
        2 => line_from_delta(from, -1, 1),
        3 => line_from_delta(from, 0, -1),
        _ => Bitboard::EMPTY,
    }
}

fn assert_state_matches_position(pos: &Position) {
    let stack = pos.state_stack();
    let state = stack.current();
    let keys = pos.compute_keys();
    assert_eq!(state.board_key, keys.board_key);
    assert_eq!(state.hand_key, keys.hand_key);
    assert_eq!(state.hand, pos.hand(pos.turn()));
}

#[test]
fn test_between_line_tables_match_yaneuraou_logic() {
    for from_idx in 0..Square::COUNT {
        let from = Square::from_index(from_idx);
        for to_idx in 0..Square::COUNT {
            let to = Square::from_index(to_idx);
            assert_eq!(Bitboard::between(from, to), between_expected(from, to));
            assert_eq!(Bitboard::line(from, to), line_expected(from, to));
        }
    }
}

#[test]
#[allow(clippy::large_stack_arrays)]
fn test_zobrist_table_matches_yaneuraou_logic() {
    let mut rng = Prng::new(ZOBRIST_SEED);
    let expected_side = next_key(&mut rng);
    let expected_no_pawns = next_key(&mut rng);

    let mut expected_board = [[ZobristKey::default(); Square::COUNT_WITH_NONE]; Piece::COUNT];
    let mut expected_hand = [[ZobristKey::default(); PieceType::HAND_TABLE_SIZE]; Color::COUNT];

    for row in expected_board.iter_mut().take(Piece::COUNT).skip(1) {
        for cell in row.iter_mut().take(Square::COUNT) {
            *cell = next_key(&mut rng);
        }
    }

    for row in expected_hand.iter_mut().take(Color::COUNT) {
        for cell in row.iter_mut().take(PieceType::HAND_TABLE_SIZE).skip(1) {
            *cell = next_key(&mut rng);
        }
    }

    let zobrist = crate::board::zobrist::Zobrist::instance();
    assert_eq!(zobrist.side(), expected_side);
    assert_eq!(zobrist.no_pawns(), expected_no_pawns);

    for (piece, row) in expected_board.iter().enumerate().take(Piece::COUNT) {
        for (sq, cell) in row.iter().enumerate().take(Square::COUNT_WITH_NONE) {
            assert_eq!(zobrist.board_at_index(piece, sq), *cell);
        }
    }

    for (color, row) in expected_hand.iter().enumerate().take(Color::COUNT) {
        for (pt, cell) in row.iter().enumerate().take(PieceType::HAND_TABLE_SIZE) {
            assert_eq!(zobrist.hand_at_index(color, pt), *cell);
        }
    }
}

#[test]
fn test_state_info_matches_position_after_capture() {
    let sfen = "ln1g3nl/1r3kg2/p2pppsp1/3s2p1p/1pp4P1/P1P1SP2P/1PSPP1P2/2GK3R1/LN3G1NL b Bb 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.init_stack();
    assert_state_matches_position(&pos);

    let mv = move_from_usi_expect(&pos, "7f7e");
    pos.apply_move32(mv);
    assert_state_matches_position(&pos);
}

#[test]
fn test_state_info_matches_position_after_drop() {
    let sfen = "4K4/9/9/9/9/9/9/9/4k4 b P 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.init_stack();
    assert_state_matches_position(&pos);

    let mv = move_from_usi_expect(&pos, "P*5e");
    pos.apply_move32(mv);
    assert_state_matches_position(&pos);
}

#[test]
fn test_state_info_matches_position_after_promotion() {
    let sfen = "lnsgkgsnl/1r5b1/2R6/ppppppppp/9/9/PPPPPPPPP/1B7/LNSGKGSNL b - 1";
    let mut pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    pos.init_stack();
    assert_state_matches_position(&pos);

    let mv = move_from_usi_expect(&pos, "7c7b+");
    pos.apply_move32(mv);
    assert_state_matches_position(&pos);
}

#[test]
fn test_state_info_matches_position_after_null_move() {
    let mut pos = crate::board::hirate_position();
    pos.init_stack();
    assert_state_matches_position(&pos);

    pos.apply_null_move().expect("null move");
    assert_state_matches_position(&pos);

    pos.undo_null_move().expect("undo null move");
    assert_state_matches_position(&pos);
}
