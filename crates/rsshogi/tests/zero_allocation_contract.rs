//! 探索から繰り返し呼ばれる経路がヒープ確保を行わないことを検査する。
//!
//! 対象は「局面を複製しない API」に限る。`Position` を複製する
//! [`rsshogi::mate::solve_mate_in_one`] のような API は state stack の確保を伴うため
//! 対象外で、同じ役割の in-place 版を検査する。

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use rsshogi::board::{
    CapturePlusPro, Checks, Legal, LegalAll, Move32List, MoveList, Position, Quiets,
    generate_legal_all_move32, generate_moves, generate_moves_move32, hirate_position,
};
use rsshogi::mate::solve_mate_in_one_in_place;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

struct CountingAllocator;

// SAFETY: 確保・解放はすべて `System` に委譲し、カウンタを進めるだけである。
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[track_caller]
fn assert_no_allocation(label: &str, body: impl FnOnce()) {
    let before = ALLOCATIONS.load(Ordering::Relaxed);
    body();
    let after = ALLOCATIONS.load(Ordering::Relaxed);
    assert_eq!(after, before, "{label} allocated {} time(s)", after - before);
}

const POSITIONS: &[&str] = &[
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1",
    "6n1l/2+S1k4/2lp4p/1np1B2b1/3PP4/1N1S3rP/1P2+pPP+p1/1p1G5/3KG2r1 b GSN2L4Pgs2p 1",
    "R8/2K1S1SSk/4B4/9/9/9/9/9/1L1L1L3 b RBGSNLP3g3n17p 1",
];

fn positions() -> Vec<Position> {
    POSITIONS.iter().map(|sfen| Position::from_sfen(sfen).expect("valid fixture")).collect()
}

fn move_generation_does_not_allocate() {
    let fixtures = positions();
    // 生成器の初期化や fixture の構築は計測区間の外で済ませる。
    let mut moves = MoveList::new();
    let mut moves32 = Move32List::new();

    assert_no_allocation("move generation", || {
        for pos in &fixtures {
            generate_moves::<Legal>(pos, &mut moves);
            generate_moves::<LegalAll>(pos, &mut moves);
            generate_moves::<Quiets>(pos, &mut moves);
            generate_moves::<CapturePlusPro>(pos, &mut moves);
            generate_moves::<Checks>(pos, &mut moves);
            generate_moves_move32::<LegalAll>(pos, &mut moves32);
            generate_legal_all_move32(pos, &mut moves32);
        }
    });
}

fn fresh_move_lists_live_on_the_stack() {
    let pos = hirate_position();
    assert_no_allocation("MoveList construction", || {
        let mut moves = MoveList::new();
        generate_moves::<LegalAll>(&pos, &mut moves);
        let mut moves32 = Move32List::new();
        generate_legal_all_move32(&pos, &mut moves32);
        assert!(!moves.is_empty() && !moves32.is_empty());
    });
}

fn apply_and_undo_do_not_allocate() {
    let mut pos = hirate_position();
    pos.init_stack();
    let mut moves = Move32List::new();
    generate_legal_all_move32(&pos, &mut moves);
    let candidates: Vec<_> = moves.iter().copied().collect();

    assert_no_allocation("apply/undo", || {
        for &mv in &candidates {
            pos.apply_move32(mv);
            pos.undo_move32(mv).expect("undo must succeed");
        }
    });
}

#[allow(deprecated)] // 1.x の search null move で予約容量と clone 後の無確保契約を検査する。
fn prepared_clone_keeps_allocation_free_capacity() {
    let mut source = hirate_position();
    source.prepare_search_state_capacity(130).unwrap();
    let mut cloned = source.clone();
    let original_key = source.key();
    assert_no_allocation("prepared clone capacity", || {
        for _ in 0..130 {
            cloned.try_apply_search_null_move().unwrap();
        }
        for _ in 0..130 {
            cloned.undo_search_null_move().unwrap();
        }
    });
    assert_eq!(cloned.key(), original_key);
    assert_eq!(source.key(), original_key);
    assert_eq!(source.state_stack_depth(), 0);
}

fn prepared_normal_moves_do_not_allocate() {
    let mut source = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
    source.prepare_search_state_capacity(130).unwrap();
    let original_key = source.key();
    let original_ply = source.game_ply();
    let mut setup = source.clone();
    let moves: Vec<_> = ["5i6i", "5a6a", "6i5i", "6a5a"]
        .into_iter()
        .cycle()
        .take(130)
        .map(|usi| {
            let mv = setup.move32_from_move(rsshogi::types::Move::from_usi(usi).unwrap());
            assert!(setup.is_legal_move32(mv));
            setup.apply_move32(mv);
            mv
        })
        .collect();
    let mut cloned = source.clone();
    assert_no_allocation("prepared normal moves", || {
        for &mv in &moves {
            assert!(cloned.has_prepared_search_state());
            cloned.apply_move32(mv);
        }
        for &mv in moves.iter().rev() {
            cloned.undo_move32(mv).unwrap();
        }
    });
    assert_eq!(cloned.key(), original_key);
    assert_eq!(cloned.game_ply(), original_ply);
    assert_eq!(cloned.state_stack_depth(), 0);
}

fn clone_from_reuses_prepared_capacity() {
    let mut source = hirate_position();
    source.prepare_search_state_capacity(130).unwrap();
    let mut target = hirate_position();
    target.prepare_search_state_capacity(260).unwrap();
    for indexed in [false, true] {
        if indexed {
            source = source.clone_for_search();
        }
        assert_no_allocation("prepared clone_from", || {
            for _ in 0..8 {
                target.clone_from(&source);
            }
            for _ in 0..130 {
                target.apply_null_move().unwrap();
            }
            for _ in 0..130 {
                target.undo_null_move().unwrap();
            }
        });
        assert_eq!(target.key(), source.key());
    }
}

fn mate_in_one_in_place_does_not_allocate() {
    let mut fixtures: Vec<Position> = positions();
    for pos in &mut fixtures {
        pos.init_stack();
    }
    let mut mate = Position::from_sfen(
        "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
    )
    .expect("valid mate fixture");
    mate.init_stack();

    // debug ビルドでは棄却候補を apply で突き合わせる健全性検査が局面を複製するため、
    // 確保が発生する。契約が成り立つのは検査の入らないビルドに限る。
    let check_allocations = !cfg!(debug_assertions);
    let run = |fixtures: &mut Vec<Position>, mate: &mut Position| {
        for pos in fixtures.iter_mut() {
            let _ = solve_mate_in_one_in_place(pos);
        }
        assert!(solve_mate_in_one_in_place(mate).is_some());
    };
    if check_allocations {
        assert_no_allocation("solve_mate_in_one_in_place", || run(&mut fixtures, &mut mate));
    } else {
        run(&mut fixtures, &mut mate);
    }
}

fn is_mated_does_not_allocate() {
    let mut fixtures = positions();
    for pos in &mut fixtures {
        pos.init_stack();
    }

    assert_no_allocation("is_mated", || {
        for pos in &fixtures {
            let _ = pos.is_mated();
        }
    });
}

/// 計測カウンタはプロセス全体で共有されるため、検査は 1 つの test に集約して直列に走らせる。
#[test]
fn hot_paths_do_not_allocate() {
    move_generation_does_not_allocate();
    fresh_move_lists_live_on_the_stack();
    apply_and_undo_do_not_allocate();
    prepared_clone_keeps_allocation_free_capacity();
    prepared_normal_moves_do_not_allocate();
    clone_from_reuses_prepared_capacity();
    mate_in_one_in_place_does_not_allocate();
    is_mated_does_not_allocate();
}
