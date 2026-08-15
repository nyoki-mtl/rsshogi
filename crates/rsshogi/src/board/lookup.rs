use crate::types::{Bitboard, Square};

const fn between_bits(from: usize, to: usize) -> u128 {
    let from_file = from / 9;
    let from_rank = from % 9;
    let to_file = to / 9;
    let to_rank = to % 9;
    let file_delta = to_file as i8 - from_file as i8;
    let rank_delta = to_rank as i8 - from_rank as i8;

    if (file_delta != 0 && rank_delta != 0 && file_delta.abs() != rank_delta.abs())
        || (file_delta == 0 && rank_delta == 0)
    {
        return 0;
    }

    let file_distance = file_delta.abs();
    let rank_distance = rank_delta.abs();
    let steps = if file_distance > rank_distance { file_distance } else { rank_distance };
    if steps <= 1 {
        return 0;
    }

    let file_step = file_delta.signum();
    let rank_step = rank_delta.signum();
    let mut bits = 0u128;
    let mut step = 1;
    while step < steps {
        let file = from_file as i8 + file_step * step;
        let rank = from_rank as i8 + rank_step * step;
        bits |= 1u128 << (file * 9 + rank);
        step += 1;
    }
    bits
}

const fn line_bits(from: usize, to: usize) -> u128 {
    let from_file = from / 9;
    let from_rank = from % 9;
    let to_file = to / 9;
    let to_rank = to % 9;
    let file_delta = to_file as i8 - from_file as i8;
    let rank_delta = to_rank as i8 - from_rank as i8;

    if file_delta == 0 && rank_delta == 0 {
        return 1u128 << from;
    }

    if file_delta != 0 && rank_delta != 0 && file_delta.abs() != rank_delta.abs() {
        return 0;
    }

    let file_step = file_delta.signum();
    let rank_step = rank_delta.signum();
    let mut file = from_file as i8;
    let mut rank = from_rank as i8;
    while file - file_step >= 0
        && file - file_step < 9
        && rank - rank_step >= 0
        && rank - rank_step < 9
    {
        file -= file_step;
        rank -= rank_step;
    }

    let mut bits = 0u128;
    while file >= 0 && file < 9 && rank >= 0 && rank < 9 {
        bits |= 1u128 << (file * 9 + rank);
        file += file_step;
        rank += rank_step;
        if file_step == 0 && rank_step == 0 {
            break;
        }
    }
    bits
}

const fn build_between() -> [[u128; 81]; 81] {
    let mut table = [[0u128; 81]; 81];
    let mut from = 0;
    while from < 81 {
        let mut to = 0;
        while to < 81 {
            table[from][to] = between_bits(from, to);
            to += 1;
        }
        from += 1;
    }
    table
}

const fn build_line() -> [[u128; 81]; 81] {
    let mut table = [[0u128; 81]; 81];
    let mut from = 0;
    while from < 81 {
        let mut to = 0;
        while to < 81 {
            table[from][to] = line_bits(from, to);
            to += 1;
        }
        from += 1;
    }
    table
}

static BETWEEN: [[u128; 81]; 81] = build_between();
static LINE: [[u128; 81]; 81] = build_line();

impl Bitboard {
    /// Warms geometry lookup data.
    ///
    /// The tables are compile-time constants, so accessing them requires no
    /// runtime initialization.
    pub fn init_tables() {}

    /// Returns the strictly interior squares when two squares share a board
    /// line, otherwise an empty bitboard.
    #[must_use]
    #[inline]
    pub fn between(from: Square, to: Square) -> Self {
        Self::from_packed_bits(BETWEEN[from.to_index()][to.to_index()])
    }

    /// Returns the complete board line through two collinear squares.
    #[must_use]
    #[inline]
    pub fn line(from: Square, to: Square) -> Self {
        Self::from_packed_bits(LINE[from.to_index()][to.to_index()])
    }

    /// Returns whether `from` and `to` lie on the same ray from `king`.
    #[must_use]
    #[inline]
    pub fn is_aligned(from: Square, to: Square, king: Square) -> bool {
        let from_file = from.file().raw();
        let from_rank = from.rank().raw();
        let to_file = to.file().raw();
        let to_rank = to.rank().raw();
        let king_file = king.file().raw();
        let king_rank = king.rank().raw();

        let collinear = (from_file == to_file && to_file == king_file)
            || (from_rank == to_rank && to_rank == king_rank)
            || (from_file - from_rank == to_file - to_rank
                && to_file - to_rank == king_file - king_rank)
            || (from_file + from_rank == to_file + to_rank
                && to_file + to_rank == king_file + king_rank);
        let from_file_delta = from_file - king_file;
        let from_rank_delta = from_rank - king_rank;
        let to_file_delta = to_file - king_file;
        let to_rank_delta = to_rank - king_rank;
        collinear && (from_file_delta * to_file_delta + from_rank_delta * to_rank_delta) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar_between(from: Square, to: Square) -> Bitboard {
        let from_file = from.raw() / 9;
        let from_rank = from.raw() % 9;
        let to_file = to.raw() / 9;
        let to_rank = to.raw() % 9;
        let file_delta = to_file - from_file;
        let rank_delta = to_rank - from_rank;

        if (file_delta == 0 && rank_delta == 0)
            || (file_delta != 0 && rank_delta != 0 && file_delta.abs() != rank_delta.abs())
        {
            return Bitboard::EMPTY;
        }

        let file_distance = file_delta.abs();
        let rank_distance = rank_delta.abs();
        let steps = if file_distance > rank_distance { file_distance } else { rank_distance };
        if steps <= 1 {
            return Bitboard::EMPTY;
        }

        let mut result = Bitboard::EMPTY;
        for step in 1..steps {
            let file = from_file + file_delta.signum() * step;
            let rank = from_rank + rank_delta.signum() * step;
            result.set(Square::new(file * 9 + rank));
        }
        result
    }

    #[test]
    fn between_matches_scalar_for_all_square_pairs() {
        for from_raw in 0..81 {
            for to_raw in 0..81 {
                let from = Square::new(from_raw);
                let to = Square::new(to_raw);
                assert_eq!(Bitboard::between(from, to), scalar_between(from, to));
            }
        }
    }

    #[test]
    fn is_aligned_recognizes_each_board_line() {
        assert!(Bitboard::is_aligned(Square::new(0), Square::new(4), Square::new(8)));
        assert!(Bitboard::is_aligned(Square::new(0), Square::new(36), Square::new(72)));
        assert!(Bitboard::is_aligned(Square::new(0), Square::new(40), Square::new(80)));
        assert!(Bitboard::is_aligned(Square::new(8), Square::new(40), Square::new(72)));
        assert!(!Bitboard::is_aligned(Square::new(0), Square::new(1), Square::new(9)));
        assert!(!Bitboard::is_aligned(Square::new(0), Square::new(8), Square::new(4)));
    }
}
