use crate::types::{Bitboard, Square};
use std::sync::OnceLock;

const BETWEEN_BB_TABLE_SIZE: usize = 785;

struct BetweenTables {
    index: [[u16; Square::COUNT_WITH_NONE]; Square::COUNT_WITH_NONE],
    bb: [Bitboard; BETWEEN_BB_TABLE_SIZE],
}

struct DirectionTables {
    mask: [[u8; Square::COUNT_WITH_NONE]; Square::COUNT_WITH_NONE],
}

struct LineTables {
    bb: [[Bitboard; 4]; Square::COUNT],
}

static BETWEEN_TABLE: OnceLock<BetweenTables> = OnceLock::new();
static DIRECTION_TABLE: OnceLock<DirectionTables> = OnceLock::new();
static LINE_TABLE: OnceLock<LineTables> = OnceLock::new();

const DIRECTIONS_RU: u8 = 1 << 0;
const DIRECTIONS_R: u8 = 1 << 1;
const DIRECTIONS_RD: u8 = 1 << 2;
const DIRECTIONS_U: u8 = 1 << 3;
const DIRECTIONS_D: u8 = 1 << 4;
const DIRECTIONS_LU: u8 = 1 << 5;
const DIRECTIONS_L: u8 = 1 << 6;
const DIRECTIONS_LD: u8 = 1 << 7;

fn build_between_table() -> BetweenTables {
    let mut index = [[0u16; Square::COUNT_WITH_NONE]; Square::COUNT_WITH_NONE];
    let mut bb = [Bitboard::EMPTY; BETWEEN_BB_TABLE_SIZE];
    let mut between_index: u16 = 1;

    for (s1, row) in index.iter_mut().enumerate().take(Square::COUNT) {
        for (s2, entry) in row.iter_mut().enumerate().take(Square::COUNT) {
            if s1 >= s2 {
                continue;
            }
            let sq1 = Square::from_index(s1);
            let sq2 = Square::from_index(s2);
            let Some(delta) = between_delta(sq1, sq2) else {
                continue;
            };
            let mut line = Bitboard::EMPTY;
            let mut sq = sq1;
            loop {
                sq = Square::new(sq.raw() + delta);
                if sq == sq2 {
                    break;
                }
                line.set(sq);
            }
            if line.is_empty() {
                continue;
            }
            let idx = usize::from(between_index);
            *entry = between_index;
            bb[idx] = line;
            between_index += 1;
        }
    }

    debug_assert_eq!(usize::from(between_index), BETWEEN_BB_TABLE_SIZE, "BetweenBB size mismatch");

    #[allow(clippy::needless_range_loop)]
    for s1 in 0..Square::COUNT {
        for s2 in 0..s1 {
            index[s1][s2] = index[s2][s1];
        }
    }

    BetweenTables { index, bb }
}

fn build_line_table() -> LineTables {
    let mut bb = [[Bitboard::EMPTY; 4]; Square::COUNT];
    for (s1, row) in bb.iter_mut().enumerate().take(Square::COUNT) {
        let sq1 = Square::from_index(s1);
        row[0] = line_from_delta(sq1, -1, -1);
        row[1] = line_from_delta(sq1, -1, 0);
        row[2] = line_from_delta(sq1, -1, 1);
        row[3] = line_from_delta(sq1, 0, -1);
    }
    LineTables { bb }
}

fn direction_mask(from: Square, to: Square) -> u8 {
    if from == to {
        return 0;
    }

    let from_file = from.raw() / 9;
    let from_rank = from.raw() % 9;
    let to_file = to.raw() / 9;
    let to_rank = to.raw() % 9;

    let file_diff = to_file - from_file;
    let rank_diff = to_rank - from_rank;

    if file_diff == 0 {
        return if rank_diff < 0 { DIRECTIONS_U } else { DIRECTIONS_D };
    }
    if rank_diff == 0 {
        return if file_diff < 0 { DIRECTIONS_R } else { DIRECTIONS_L };
    }
    if file_diff.abs() == rank_diff.abs() {
        if file_diff < 0 {
            return if rank_diff < 0 { DIRECTIONS_RU } else { DIRECTIONS_RD };
        }
        return if rank_diff < 0 { DIRECTIONS_LU } else { DIRECTIONS_LD };
    }

    0
}

fn build_direction_table() -> DirectionTables {
    let mut mask = [[0u8; Square::COUNT_WITH_NONE]; Square::COUNT_WITH_NONE];

    for (s1, row) in mask.iter_mut().enumerate().take(Square::COUNT) {
        for (s2, entry) in row.iter_mut().enumerate().take(Square::COUNT) {
            *entry = direction_mask(Square::from_index(s1), Square::from_index(s2));
        }
    }

    DirectionTables { mask }
}

#[inline]
fn between_table() -> &'static BetweenTables {
    BETWEEN_TABLE.get().unwrap_or_else(init_between_table)
}

#[inline]
fn direction_table() -> &'static DirectionTables {
    DIRECTION_TABLE.get().unwrap_or_else(init_direction_table)
}

#[inline]
fn line_table() -> &'static LineTables {
    LINE_TABLE.get().unwrap_or_else(init_line_table)
}

#[cold]
#[inline(never)]
fn init_between_table() -> &'static BetweenTables {
    BETWEEN_TABLE.get_or_init(build_between_table)
}

#[cold]
#[inline(never)]
fn init_direction_table() -> &'static DirectionTables {
    DIRECTION_TABLE.get_or_init(build_direction_table)
}

#[cold]
#[inline(never)]
fn init_line_table() -> &'static LineTables {
    LINE_TABLE.get_or_init(build_line_table)
}

fn between_delta(from: Square, to: Square) -> Option<i8> {
    if from == to {
        return None;
    }
    let from_file = from.raw() / 9;
    let from_rank = from.raw() % 9;
    let to_file = to.raw() / 9;
    let to_rank = to.raw() % 9;

    let file_diff = to_file - from_file;
    let rank_diff = to_rank - from_rank;

    if file_diff == 0 {
        return Some(rank_diff.signum());
    }
    if rank_diff == 0 {
        return Some(file_diff.signum() * 9);
    }
    if file_diff.abs() == rank_diff.abs() {
        return Some(file_diff.signum() * 9 + rank_diff.signum());
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

const fn line_direction_index(from: Square, to: Square) -> Option<usize> {
    let from_file = from.raw() / 9;
    let from_rank = from.raw() % 9;
    let to_file = to.raw() / 9;
    let to_rank = to.raw() % 9;

    let file_diff = to_file - from_file;
    let rank_diff = to_rank - from_rank;

    if file_diff == 0 {
        return Some(3);
    }
    if rank_diff == 0 {
        return Some(1);
    }
    if file_diff == rank_diff || file_diff == -rank_diff {
        let same_sign = (file_diff > 0 && rank_diff > 0) || (file_diff < 0 && rank_diff < 0);
        return Some(if same_sign { 0 } else { 2 });
    }
    None
}

fn is_aligned(sq1: Square, sq2: Square, sq3: Square) -> bool {
    let table = direction_table();
    let d1 = table.mask[sq1.to_index_with_none()][sq3.to_index_with_none()];
    d1 != 0 && d1 == table.mask[sq2.to_index_with_none()][sq3.to_index_with_none()]
}

#[inline]
pub(super) fn between(from: Square, to: Square) -> Bitboard {
    debug_assert!(from.is_on_board(), "between() requires on-board square: {from:?}");
    debug_assert!(to.is_on_board(), "between() requires on-board square: {to:?}");
    let tables = between_table();
    // `COUNT_WITH_NONE` 相当のテーブル参照は `SQ_NONE` を含めて index 化する。
    tables.bb[usize::from(tables.index[from.to_index_with_none()][to.to_index_with_none()])]
}

#[inline]
pub(super) fn line(sq1: Square, sq2: Square) -> Bitboard {
    debug_assert!(sq1.is_on_board(), "line() requires on-board square: {sq1:?}");
    debug_assert!(sq2.is_on_board(), "line() requires on-board square: {sq2:?}");
    if sq1 == sq2 {
        return Bitboard::from_square(sq1);
    }
    let Some(dir) = line_direction_index(sq1, sq2) else {
        return Bitboard::EMPTY;
    };
    line_table().bb[sq1.to_index()][dir]
}

pub(super) fn init_lookup_tables() {
    let _ = between_table();
    let _ = direction_table();
    let _ = line_table();
}

impl Bitboard {
    /// 2つのマスの間のマスを表すビットボードを取得（端点は含まない）
    #[must_use]
    pub fn between(from: Square, to: Square) -> Self {
        debug_assert!(from.is_on_board(), "Invalid square: {from:?}");
        debug_assert!(to.is_on_board(), "Invalid square: {to:?}");
        between(from, to)
    }

    /// 2 つのマスを結ぶ直線上の全マスを返す（両端を含む）。
    ///
    /// 2 つのマスが同じ直線上（縦・横・斜め）にあれば、その直線上の全マスの `Bitboard` を返す。
    /// 同一直線上にない場合は空の `Bitboard` を返す。
    #[must_use]
    pub fn line(sq1: Square, sq2: Square) -> Self {
        debug_assert!(sq1.is_on_board(), "Invalid square: {sq1:?}");
        debug_assert!(sq2.is_on_board(), "Invalid square: {sq2:?}");
        line(sq1, sq2)
    }

    /// 3つのマスが同一直線上（縦・横・斜め）にあるかを返す。
    #[must_use]
    pub fn is_aligned(sq1: Square, sq2: Square, sq3: Square) -> bool {
        debug_assert!(sq1.is_on_board(), "Invalid square: {sq1:?}");
        debug_assert!(sq2.is_on_board(), "Invalid square: {sq2:?}");
        debug_assert!(sq3.is_on_board(), "Invalid square: {sq3:?}");
        is_aligned(sq1, sq2, sq3)
    }

    /// `between`/`line` テーブルを事前計算する
    pub fn init_tables() {
        init_lookup_tables();
    }
}
