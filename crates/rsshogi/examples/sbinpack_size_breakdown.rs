use rsshogi::records::formats::{pack, sbinpack};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{Error as IoError, ErrorKind};
use std::path::PathBuf;
use std::process;

#[derive(Debug)]
struct Options {
    input: PathBuf,
    stem_score: i16,
    limit: Option<usize>,
}

#[derive(Default)]
struct Totals {
    games: usize,
    plies: usize,
    legacy_score_bytes: usize,
    v2_score_bytes: usize,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let options = parse_options()?;
    let input = fs::read(&options.input)?;

    let mut offset = 0usize;
    let mut chains = Vec::new();
    let mut totals = Totals::default();

    while offset < input.len() {
        if options.limit.is_some_and(|limit| totals.games >= limit) {
            break;
        }

        let (game, consumed) = pack::decode_game(&input[offset..])?;
        offset += consumed;
        totals.games += 1;
        totals.plies += game.plies.len();
        totals.legacy_score_bytes += legacy_score_bytes(&game, i32::from(options.stem_score));
        totals.v2_score_bytes += v2_score_bytes(&game, i32::from(options.stem_score));

        let record = pack::record_from_game(&game)?;
        let chain = sbinpack::chain_from_record(&record, options.stem_score)
            .map_err(|err| IoError::other(format!("sbinpack chain conversion failed: {err:?}")))?;
        chains.push(chain);
    }

    let sbinpack_bytes = sbinpack::serialize_file(&chains)
        .map_err(|err| IoError::other(format!("sbinpack serialization failed: {err:?}")))?;

    print_report(&options, &input, offset, sbinpack_bytes.len(), &totals);
    Ok(())
}

fn parse_options() -> Result<Options, Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let mut input = None;
    let mut stem_score = 0i16;
    let mut limit = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_usage();
                process::exit(0);
            }
            "--stem-score" => {
                let value =
                    args.next().ok_or_else(|| invalid_input("--stem-score requires a value"))?;
                stem_score = value.parse()?;
            }
            "--limit" => {
                let value = args.next().ok_or_else(|| invalid_input("--limit requires a value"))?;
                limit = Some(value.parse()?);
            }
            _ if input.is_none() => input = Some(PathBuf::from(arg)),
            _ => return Err(invalid_input(format!("unexpected argument: {arg}")).into()),
        }
    }

    let input = input.ok_or_else(|| invalid_input("missing input .pack path"))?;
    Ok(Options { input, stem_score, limit })
}

fn invalid_input(message: impl Into<String>) -> IoError {
    IoError::new(ErrorKind::InvalidInput, message.into())
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p rsshogi --example sbinpack_size_breakdown -- <input.pack> [--stem-score N] [--limit N]"
    );
}

fn legacy_score_bytes(game: &pack::PackGame, stem_score: i32) -> usize {
    let mut bytes = 0usize;
    let mut prev = stem_score;
    for ply in &game.plies {
        let current = i32::from(ply.eval);
        bytes += zigzag_uleb_len(current - prev);
        prev = current;
    }
    bytes
}

fn v2_score_bytes(game: &pack::PackGame, stem_score: i32) -> usize {
    let mut bytes = 0usize;
    let mut prev = stem_score;
    for (index, ply) in game.plies.iter().enumerate() {
        let current = normalize_eval_for_delta(i32::from(ply.eval), index);
        bytes += zigzag_uleb_len(current - prev);
        prev = current;
    }
    bytes
}

fn normalize_eval_for_delta(eval: i32, ply_index: usize) -> i32 {
    if ply_index.is_multiple_of(2) { eval } else { -eval }
}

fn zigzag_uleb_len(value: i32) -> usize {
    uleb_len(zigzag_i32(value))
}

fn zigzag_i32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

fn uleb_len(mut value: u32) -> usize {
    let mut len = 1usize;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

fn print_report(
    options: &Options,
    input: &[u8],
    consumed_pack_bytes: usize,
    sbinpack_bytes: usize,
    totals: &Totals,
) {
    println!("sbinpack size breakdown");
    println!("input: {}", options.input.display());
    println!("stem_score: {}", options.stem_score);
    println!("games: {}", totals.games);
    println!("plies: {}", totals.plies);
    println!("pack_bytes_scanned: {consumed_pack_bytes}");
    println!("pack_bytes_total: {}", input.len());
    println!("sbinpack_v2_bytes: {sbinpack_bytes}");
    println!("legacy_score_bytes: {}", totals.legacy_score_bytes);
    println!("v2_score_bytes: {}", totals.v2_score_bytes);

    if totals.plies > 0 {
        println!(
            "legacy_score_bytes_per_ply: {:.4}",
            totals.legacy_score_bytes as f64 / totals.plies as f64
        );
        println!(
            "v2_score_bytes_per_ply: {:.4}",
            totals.v2_score_bytes as f64 / totals.plies as f64
        );
    }
    if totals.legacy_score_bytes > 0 {
        println!(
            "score_byte_ratio_v2_over_legacy: {:.4}",
            totals.v2_score_bytes as f64 / totals.legacy_score_bytes as f64
        );
    }
    if consumed_pack_bytes > 0 {
        println!(
            "file_byte_ratio_v2_over_pack_scanned: {:.4}",
            sbinpack_bytes as f64 / consumed_pack_bytes as f64
        );
    }
}
