use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rsshogi::{
    board::attack_tables::{bishop_attacks, lance_attacks, rook_attacks},
    types::{Bitboard, Color, Square},
};

const OCCUPANCY: Bitboard = Bitboard::from_packed_bits(0x1_2345_6789_abcd_ef01_2345);

fn bench_slider_attacks(c: &mut Criterion) {
    let squares: Vec<_> = Square::iter().collect();
    let mut group = c.benchmark_group("slider_attacks_all_squares");

    group.bench_function("bishop", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= bishop_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("rook", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= rook_attacks(square, black_box(OCCUPANCY));
            }
            black_box(attacks)
        });
    });
    group.bench_function("lance", |b| {
        b.iter(|| {
            let mut attacks = Bitboard::EMPTY;
            for &square in &squares {
                attacks |= lance_attacks(square, black_box(OCCUPANCY), Color::BLACK);
                attacks |= lance_attacks(square, black_box(OCCUPANCY), Color::WHITE);
            }
            black_box(attacks)
        });
    });
    group.finish();
}

criterion_group!(attack_table_benches, bench_slider_attacks);
criterion_main!(attack_table_benches);
