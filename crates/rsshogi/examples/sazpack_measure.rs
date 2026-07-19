//! SAZ2 codecの簡易throughput計測。

use std::time::Instant;

use rsshogi::{
    board,
    records::formats::sazpack::{
        SazOutcomeBound, SazSelfplayGame, SazSelfplayPolicyEntry, SazSelfplayPosition,
        SazTerminationReason, SazWdl, deserialize_selfplay_chunk, serialize_selfplay_chunk,
    },
    types::{EnteringKingRule, GameResult, Move},
};

fn main() {
    board::init();
    let games = vec![fixture(); 1_000];
    let started = Instant::now();
    let bytes = serialize_selfplay_chunk(&games).expect("serialize SAZ2");
    let encode_elapsed = started.elapsed();
    let started = Instant::now();
    let decoded = deserialize_selfplay_chunk(&bytes).expect("deserialize SAZ2");
    let decode_elapsed = started.elapsed();

    println!("games={} bytes={}", decoded.len(), bytes.len());
    println!("encode_ms={}", encode_elapsed.as_millis());
    println!("decode_ms={}", decode_elapsed.as_millis());
}

fn fixture() -> SazSelfplayGame {
    SazSelfplayGame {
        stem_packed_sfen: board::hirate_position().to_packed_sfen(),
        game_result: GameResult::DrawByMaxPlies,
        termination_reason: SazTerminationReason::MaxGamePlies,
        entering_king_rule: EnteringKingRule::Point27,
        positions: vec![SazSelfplayPosition {
            played: Move::from_usi("7g7f").unwrap(),
            root_wdl: SazWdl { win: 20_000, draw: 25_535, loss: 20_000 },
            outcome_wdl: SazWdl { win: 0, draw: u16::MAX, loss: 0 },
            plies_left: 1,
            requested_visits: 800,
            target_weight_milli: 1_000,
            exploration_flags: 3,
            mate: None,
            policy: vec![
                SazSelfplayPolicyEntry {
                    mv: Move::from_usi("7g7f").unwrap(),
                    prior: 50_000,
                    visits_before: 0,
                    visits_after: 600,
                    lower: SazOutcomeBound::Loss,
                    upper: SazOutcomeBound::Win,
                },
                SazSelfplayPolicyEntry {
                    mv: Move::from_usi("2g2f").unwrap(),
                    prior: 15_535,
                    visits_before: 0,
                    visits_after: 200,
                    lower: SazOutcomeBound::Loss,
                    upper: SazOutcomeBound::Win,
                },
            ],
        }],
    }
}
