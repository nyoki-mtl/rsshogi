//! テストビルド専用の Native I/O 区間計測。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Default, Debug)]
pub(super) struct Measurement {
    pub calls: u64,
    pub units: u64,
    pub elapsed: Duration,
}

thread_local! {
    static ACTIVE: RefCell<Option<BTreeMap<&'static str, Measurement>>> = const { RefCell::new(None) };
}

pub(super) fn start() {
    ACTIVE.with(|active| *active.borrow_mut() = Some(BTreeMap::new()));
}

pub(super) fn finish() -> BTreeMap<&'static str, Measurement> {
    ACTIVE.with(|active| active.borrow_mut().take().expect("profile is active"))
}

pub(super) struct Timer(Option<(Instant, &'static str, u64)>);

pub(super) fn measure(phase: &'static str, units: u64) -> Timer {
    Timer(ACTIVE.with(|active| active.borrow().as_ref().map(|_| (Instant::now(), phase, units))))
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some((start, phase, units)) = self.0 {
            let elapsed = start.elapsed();
            ACTIVE.with(|active| {
                if let Some(active) = active.borrow_mut().as_mut() {
                    let entry = active.entry(phase).or_default();
                    entry.calls += 1;
                    entry.units += units;
                    entry.elapsed += elapsed;
                }
            });
        }
    }
}

#[test]
#[ignore = "set RSSHOGI_NATIVE_PROFILE_INPUT and RSSHOGI_NATIVE_PROFILE_OUTPUT"]
fn native_io_profile() {
    use super::{NativeBook, NativeBookBuilder};
    let input = std::env::var_os("RSSHOGI_NATIVE_PROFILE_INPUT").expect("input path");
    let output = std::env::var_os("RSSHOGI_NATIVE_PROFILE_OUTPUT").expect("new output path");
    let book = NativeBook::open(input).unwrap();
    book.validate_full().unwrap();
    let records = book.records().collect::<Result<Vec<_>, _>>().unwrap();
    let mut builder = NativeBookBuilder::new(output, book.metadata().unwrap())
        .unwrap()
        .with_memory_budget(1024 * 1024)
        .unwrap();
    let mut count = 0;
    start();
    let push_start = Instant::now();
    for record in records {
        builder.push(record).unwrap();
        count += 1;
    }
    let push = push_start.elapsed();
    let push_phases = finish();
    start();
    let finish_start = Instant::now();
    builder.finish().unwrap();
    let finish_elapsed = finish_start.elapsed();
    let finish_phases = finish();
    println!("positions={count} push={push:?} finish={finish_elapsed:?}");
    println!("push phases={push_phases:#?}\nfinish phases={finish_phases:#?}");
}

#[test]
#[ignore = "set RSSHOGI_NATIVE_PROFILE_INPUT"]
fn native_visit_profile() {
    use super::NativeBook;
    use crate::board::{PackedSfen, Position};
    use crate::book::BookControl;
    let input = std::env::var_os("RSSHOGI_NATIVE_PROFILE_INPUT").expect("input path");
    let mut expected = None;
    // 最初の一組を warmup として記録し、残りを交互順で比較する。
    for repetition in 0..6 {
        let modes = if repetition % 2 == 0 { [false, true] } else { [true, false] };
        for combined in modes {
            let book = NativeBook::open(&input).unwrap();
            let mut counts = [0_u64; 3];
            let mut visit = |entry: super::NativeBookEntry, position: &crate::board::Position| {
                counts[0] += 1;
                counts[1] += entry.candidates.len() as u64;
                counts[2] = counts[2].wrapping_add(
                    u64::from_le_bytes(entry.key[..8].try_into().unwrap()) ^ position.turn() as u64,
                );
                Ok(())
            };
            let start = Instant::now();
            if combined {
                book.validate_full_and_visit(&mut visit, |_| BookControl::Continue).unwrap();
            } else {
                book.validate_full().unwrap();
                let mut position = Position::empty();
                for entry in book.entries() {
                    let entry = entry.unwrap();
                    position.set_packed_sfen(&PackedSfen::from_bytes(entry.key), false, 1).unwrap();
                    visit(entry, &position).unwrap();
                }
            }
            let elapsed = start.elapsed();
            assert_eq!(*expected.get_or_insert(counts), counts);
            println!(
                "repetition={repetition} combined={combined} elapsed={elapsed:?} counts={counts:?}"
            );
        }
    }
}
