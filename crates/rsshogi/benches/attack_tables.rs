use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rsshogi::{
    board::attack_tables::{
        BISHOP_BEAMS, LANCE_BEAMS, ROOK_BEAMS, bishop_attacks, lance_attacks, rook_attacks,
    },
    types::{Bitboard, Color, Square},
};

const OCCUPANCY: Bitboard = Bitboard::from_packed_bits(0x1_2345_6789_abcd_ef01_2345);

fn legacy_ray_attacks(ray: Bitboard, occupied: Bitboard, increasing: bool) -> Bitboard {
    let ray_bits = ray.packed_bits();
    let blockers = ray_bits & occupied.packed_bits();
    if blockers == 0 {
        return ray;
    }

    let attacks = if increasing {
        let blocker = blockers.trailing_zeros();
        ray_bits & ((1u128 << (blocker + 1)) - 1)
    } else {
        let blocker = 127 - blockers.leading_zeros();
        ray_bits & !((1u128 << blocker) - 1)
    };
    Bitboard::from_packed_bits(attacks)
}

fn legacy_bishop_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let beams = BISHOP_BEAMS[square];
    legacy_ray_attacks(beams.ne, occupied, true)
        | legacy_ray_attacks(beams.se, occupied, true)
        | legacy_ray_attacks(beams.sw, occupied, false)
        | legacy_ray_attacks(beams.nw, occupied, false)
}

fn legacy_rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    let beams = ROOK_BEAMS[square];
    legacy_ray_attacks(beams.n, occupied, false)
        | legacy_ray_attacks(beams.e, occupied, true)
        | legacy_ray_attacks(beams.s, occupied, true)
        | legacy_ray_attacks(beams.w, occupied, false)
}

fn legacy_lance_attacks(square: Square, occupied: Bitboard, color: Color) -> Bitboard {
    let beams = LANCE_BEAMS[square];
    let ray = match color {
        Color::BLACK => beams.black,
        Color::WHITE => beams.white,
    };
    legacy_ray_attacks(ray, occupied, color == Color::WHITE)
}

fn bench_slider_attacks(c: &mut Criterion) {
    let squares: Vec<_> = Square::iter().collect();
    let mut group = c.benchmark_group("slider_attacks_all_squares");

    group.bench_function("bishop_candidate", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= bishop_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("bishop_legacy", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= legacy_bishop_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("rook_candidate", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= rook_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("rook_legacy", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= legacy_rook_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("lance_candidate", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= lance_attacks(square, black_box(OCCUPANCY), Color::BLACK);
                attacks |= lance_attacks(square, black_box(OCCUPANCY), Color::WHITE);
            }
            black_box(attacks)
        });
    });
    group.bench_function("lance_legacy", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= legacy_lance_attacks(square, black_box(OCCUPANCY), Color::BLACK);
                attacks |= legacy_lance_attacks(square, black_box(OCCUPANCY), Color::WHITE);
            }
            black_box(attacks)
        });
    });
    group.finish();
}

criterion_group!(attack_table_benches, bench_slider_attacks);
criterion_main!(attack_table_benches);
