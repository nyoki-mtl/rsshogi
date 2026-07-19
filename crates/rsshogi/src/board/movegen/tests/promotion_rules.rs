use crate::board::{MoveList, movegen};

#[test]
fn test_bishop_in_enemy_territory_only_promotes_in_search_moves() {
    let pos = crate::board::position_from_sfen("k8/4B4/9/9/9/9/9/9/4K4 b - 1").expect("parse SFEN");

    let mut list = MoveList::new();
    movegen::generate_moves::<movegen::NonEvasions>(&pos, &mut list);

    let mut from_bishop = Vec::new();
    for mv in list.iter() {
        let usi = mv.to_usi();
        if usi.starts_with("5b") {
            from_bishop.push(usi);
        }
    }

    assert!(!from_bishop.is_empty(), "bishop moves must be generated");
    assert!(
        from_bishop.iter().all(|mv| mv.ends_with('+')),
        "bishop moves from enemy territory must be promotions in search movegen"
    );
}

#[test]
fn test_lance_non_promote_restricted_in_search_moves() {
    let pos = crate::board::position_from_sfen("k8/9/4L4/9/9/9/9/9/4K4 b - 1").expect("parse SFEN");

    let mut list = MoveList::new();
    movegen::generate_moves::<movegen::NonEvasions>(&pos, &mut list);

    let mut from_lance = Vec::new();
    for mv in list.iter() {
        let usi = mv.to_usi();
        if usi.starts_with("5c") {
            from_lance.push(usi);
        }
    }

    assert!(!from_lance.is_empty(), "lance moves must be generated");
    assert!(
        !from_lance.iter().any(|mv| mv == "5c5b" || mv == "5c5a"),
        "lance non-promotion moves to rank1/2 must not be generated in search movegen"
    );
    let has_5c5b_promote = from_lance.iter().any(|mv| mv == "5c5b+");
    let has_5c5a_plus = from_lance.iter().any(|mv| mv == "5c5a+");
    assert!(
        has_5c5b_promote && has_5c5a_plus,
        "lance promotion moves to rank1/2 must be generated"
    );
}
