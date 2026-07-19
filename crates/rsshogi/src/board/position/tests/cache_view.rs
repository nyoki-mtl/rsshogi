use super::helpers::apply_usi_move;
use crate::board::position_from_sfen;
use crate::types::{Color, PieceType};

fn check_sfens() -> &'static [&'static str] {
    &[
        // 平手初期局面（王手なし）
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        // 中盤的な局面
        "ln1g3nl/1r3kg2/p2pppsp1/3s2p1p/1pp4P1/P1P1SP2P/1PSPP1P2/2GK3R1/LN3G1NL b Bb 1",
        // 王手がかかっている局面（簡素な例）
        "9/9/9/4k4/9/9/9/4K4/4R4 b - 1",
    ]
}

fn assert_cache_matches_position(pos: &crate::board::position::Position, label: &str) {
    let cache = pos.current_state_cache();

    assert_eq!(cache.checkers(), pos.checkers(), "checkers mismatch after {label}");
    for color in [Color::BLACK, Color::WHITE] {
        assert_eq!(
            cache.blockers_for_king(color),
            pos.blockers_for_king(color),
            "blockers mismatch ({color:?}) after {label}"
        );
        assert_eq!(
            cache.pinners(color),
            pos.pinners(color),
            "pinners mismatch ({color:?}) after {label}"
        );
    }
    for pt in PieceType::iter() {
        assert_eq!(
            cache.check_square(pt),
            pos.check_square(pt),
            "check_square mismatch ({pt:?}) after {label}"
        );
    }

    let view_ptr = std::ptr::from_ref(cache.check_squares());
    let getter_ptr = std::ptr::from_ref(pos.check_squares_cache());
    assert_eq!(view_ptr, getter_ptr, "check_squares alias mismatch after {label}");
}

/// `current_state_cache()` 経由の値が既存個別 getter と完全一致することを検証。
#[test]
fn cache_view_matches_individual_getters() {
    for sfen in check_sfens() {
        let pos = position_from_sfen(sfen).expect("valid sfen");
        assert_cache_matches_position(&pos, sfen);
    }
}

/// `check_squares()` は圧縮 cache 全体への共有参照を返し、コピーを発生させない。
/// アドレス比較で確認する。
#[test]
fn cache_view_check_squares_returns_borrow_of_state() {
    let pos = crate::board::hirate_position();
    let cache = pos.current_state_cache();

    let view_ptr = std::ptr::from_ref(cache.check_squares());
    let getter_ptr = std::ptr::from_ref(pos.check_squares_cache());
    assert_eq!(view_ptr, getter_ptr, "check_squares() must alias check_squares_cache()");
}

/// `apply_move32` 後でも、再取得した cache view は新しい current state を反映する。
/// （safe Rust では view を stash できないことが保証される。）
#[test]
fn cache_view_reflects_current_state_after_apply() {
    let mut pos = crate::board::hirate_position();
    let initial_checkers = pos.current_state_cache().checkers();

    apply_usi_move(&mut pos, "7g7f");

    let after_cache = pos.current_state_cache();
    assert_eq!(after_cache.checkers(), pos.checkers());
    // 平手の最初の手では王手は発生しない
    assert!(after_cache.checkers().is_empty());
    // 値そのものは初期と同じ（共に空）だが、参照する state entry は別
    assert_eq!(initial_checkers, after_cache.checkers());
}

/// 駒取り、成り、応手取り、駒打ち、玉移動を含む手順後も cache view 全体が一致する。
#[test]
fn cache_view_matches_individual_getters_after_tactical_sequence() {
    let mut pos = crate::board::hirate_position();

    for mv in ["7g7f", "3c3d", "8h3c+", "2b3c", "5g5f", "B*8e", "5i6h"] {
        apply_usi_move(&mut pos, mv);
        assert_cache_matches_position(&pos, mv);
    }
}

/// `StateCacheView` は `Copy` であり、複数 accessor を 1 つの取得から呼べる。
#[test]
fn cache_view_is_copy_and_supports_multiple_accessors() {
    let pos = crate::board::hirate_position();
    let cache = pos.current_state_cache();

    // copy されて使えることを確認
    let _checkers = cache.checkers();
    let _blockers_b = cache.blockers_for_king(Color::BLACK);
    let _pinners_b = cache.pinners(Color::BLACK);
    let _check_square_pawn = cache.check_square(PieceType::PAWN);
    let _check_squares_ref = cache.check_squares();
}
