use super::*;

fn compare(payload: &[u8], moves: &mut Vec<RawSbkMove>) {
    match (decode_state_payload(payload), decode(payload, moves)) {
        (Ok(full), Ok(position)) => {
            assert_eq!(full.position.as_deref(), position);
            let values = |moves: &[RawSbkMove]| {
                moves
                    .iter()
                    .map(|m| (m.raw_move, m.evaluation, m.weight, m.next_state_id))
                    .collect::<Vec<_>>()
            };
            assert_eq!(values(&full.moves), values(moves));
        }
        (Err(full), Err(index)) => assert_eq!(format!("{full:?}"), format!("{index:?}")),
        (full, index) => panic!("decoder mismatch: {full:?} {index:?}"),
    }
}

#[test]
fn index_decode_matches_full_decode_and_truncations() {
    let mut payload = Vec::new();
    for field in [FIELD_STATE_ID, FIELD_STATE_GAMES, FIELD_STATE_WON_BLACK, FIELD_STATE_WON_WHITE] {
        write_field_i32(&mut payload, field, -12);
    }
    write_field_string(&mut payload, FIELD_STATE_POSITION, InitialPosition::Standard.to_sfen())
        .unwrap();
    write_field_string(&mut payload, FIELD_STATE_COMMENT, "局面コメント").unwrap();
    let mut candidate = Vec::new();
    for field in
        [FIELD_MOVE_MOVE, FIELD_MOVE_EVALUATION, FIELD_MOVE_WEIGHT, FIELD_MOVE_NEXT_STATE_ID]
    {
        write_field_i32(&mut candidate, field, 0);
    }
    write_field_message(&mut payload, FIELD_STATE_MOVES, &candidate).unwrap();
    write_field_message(&mut payload, FIELD_STATE_MOVES, &candidate).unwrap();
    let mut eval = Vec::new();
    for field in [FIELD_EVAL_VALUE, FIELD_EVAL_DEPTH, FIELD_EVAL_SEL_DEPTH, FIELD_EVAL_NODES] {
        write_field_i64(&mut eval, field, -1);
    }
    write_field_string(&mut eval, FIELD_EVAL_VARIATION, "7g7f 3c3d").unwrap();
    write_field_string(&mut eval, FIELD_EVAL_ENGINE_NAME, "engine").unwrap();
    write_field_message(&mut payload, FIELD_STATE_EVALS, &eval).unwrap();
    write_field_string(&mut payload, FIELD_STATE_POSITION, "4k4/9/9/9/9/9/9/9/4K4 w - 1").unwrap();
    write_field_string(&mut payload, 99, "unknown").unwrap();
    let mut moves = Vec::new();
    for end in 0..=payload.len() {
        compare(&payload[..end], &mut moves);
    }
    let capacity = moves.capacity();
    compare(&[], &mut moves);
    assert!(moves.is_empty());
    assert_eq!(moves.capacity(), capacity);
}

#[test]
fn unused_index_fields_still_validate_utf8_varints_and_nested_wire() {
    let mut moves = Vec::new();
    for payload in [
        vec![0],
        vec![0x80; 11],
        vec![0x0b],
        vec![(FIELD_STATE_COMMENT << 3 | 2) as u8, 1, 0xff],
        vec![(FIELD_STATE_WON_WHITE << 3) as u8, 0x80],
        vec![(FIELD_STATE_EVALS << 3 | 2) as u8, 1, 0x80],
        vec![
            (FIELD_STATE_EVALS << 3 | 2) as u8,
            3,
            (FIELD_EVAL_ENGINE_NAME << 3 | 2) as u8,
            1,
            0xff,
        ],
        vec![(FIELD_STATE_EVALS << 3 | 2) as u8, 3, (FIELD_EVAL_VARIATION << 3 | 2) as u8, 1, 0xff],
        vec![(FIELD_STATE_EVALS << 3 | 2) as u8, 2, (FIELD_EVAL_NODES << 3) as u8, 0x80],
    ] {
        compare(&payload, &mut moves);
    }
}
