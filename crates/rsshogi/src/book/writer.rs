//! DB2016 テキスト定跡の書き出し（writer）。
//!
//! [`BookDatabase`] を入力に `#YANEURAOU-DB2016 1.00` テキストを生成する。writer は
//! 「素直なシリアライザ」であり、値の解釈、clamp、正規化、手番 flip は一切行わない
//! （整形と正規化は solver 側の責務）。唯一の例外として、read 側の二分探索と lookup の
//! 正当性を保証するため、**position 行は normalized SFEN（ply 除去後）の strict 昇順**に
//! 並べ替えて出力する（task 0002 Decision 5）。

use std::collections::BTreeMap;
use std::io::{self, Write};

use crate::book::{BookCandidate, BookDatabase, BookDatabaseEntry, BookError};
use crate::types::Move;

const HEADER: &str = "#YANEURAOU-DB2016 1.00";

/// 各 position 内の指し手の並び順。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MoveOrder {
    /// 評価値降順（`score == None` は末尾・同 score は元順維持の stable sort）。
    ScoreDescending,
    /// `BookDatabase` の candidate 並びをそのまま出力する。
    Preserve,
}

/// 出力する列セット（profile）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnSet {
    /// `count` / `comment` / `ponder` を保持する lossless 出力。
    Lossless,
    /// peta_shock 用の lossy profile（`move none value depth` の 4 列固定）。
    PetaShock,
}

/// position 行末尾の ply（手数）の出力方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlyMode {
    /// `original_ply` を尊重する（`Some(n>=1)`→n / `Some(0)`→省略 / `None`→1）。
    Preserve,
    /// 常に固定値を出力する。
    Fixed(u32),
    /// 末尾 ply を常に省略する（bare SFEN）。
    Omit,
}

/// normalized SFEN が重複する entry に対する挙動。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OnDuplicate {
    /// 書き込み前にエラーにする（既定）。
    Error,
    /// 後に現れた entry を採用する（後勝ちマージ）。
    KeepLast,
}

/// [`BookDatabase`] を DB2016 テキストへ書き出すときのオプション。
///
/// 既定は **lossless preserve**（`count` / `comment` / `ponder` を保持）かつ指し手は
/// **評価値降順**。position 行は常に normalized SFEN の strict 昇順に並べ替えられ、
/// これは options で変更できない（read 側の lookup 正当性に直結するため）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuDb2016WriteOptions {
    move_order: MoveOrder,
    columns: ColumnSet,
    ply: PlyMode,
    on_duplicate: OnDuplicate,
    emit_noe: bool,
}

impl Default for YaneuraOuDb2016WriteOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl YaneuraOuDb2016WriteOptions {
    /// 既定（lossless / 評価値降順 / ply preserve / 重複エラー / NOE 非出力）。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            move_order: MoveOrder::ScoreDescending,
            columns: ColumnSet::Lossless,
            ply: PlyMode::Preserve,
            on_duplicate: OnDuplicate::Error,
            emit_noe: false,
        }
    }

    /// peta_shock 用の lossy profile を生成する。
    ///
    /// `move none value depth` の 4 列に絞り、`ponder` は常に `none`、`count` / `comment`
    /// は出力しない（`write_peta_shock_book` に合わせる）。
    #[must_use]
    pub const fn peta_shock() -> Self {
        Self { columns: ColumnSet::PetaShock, ..Self::new() }
    }

    /// 指し手の並びを入力順保持（`Preserve`）にする。
    #[must_use]
    pub const fn with_preserved_move_order(mut self) -> Self {
        self.move_order = MoveOrder::Preserve;
        self
    }

    /// position 行末尾の ply を固定値で出力する。
    #[must_use]
    pub const fn with_fixed_ply(mut self, ply: u32) -> Self {
        self.ply = PlyMode::Fixed(ply);
        self
    }

    /// position 行末尾の ply を常に省略する。
    #[must_use]
    pub const fn with_omitted_ply(mut self) -> Self {
        self.ply = PlyMode::Omit;
        self
    }

    /// normalized SFEN が重複する entry を後勝ちで採用する。
    #[must_use]
    pub const fn with_keep_last_on_duplicate(mut self) -> Self {
        self.on_duplicate = OnDuplicate::KeepLast;
        self
    }

    /// 2 行目に `# NOE:<count>` を出力するかどうかを設定する。
    #[must_use]
    pub const fn with_noe(mut self, emit: bool) -> Self {
        self.emit_noe = emit;
        self
    }
}

impl BookDatabase {
    /// この定跡を DB2016 テキストとして `w` へ書き出す（ストリーミング）。
    ///
    /// position 行は normalized SFEN の strict 昇順で出力される。重複する normalized
    /// SFEN は既定ではエラーになる（[`YaneuraOuDb2016WriteOptions::with_keep_last_on_duplicate`]
    /// で後勝ちにできる）。出力を [`YaneuraOuBook::open`](crate::book::YaneuraOuBook::open)
    /// で読み戻すと診断が `Sorted { complete: true }` になる。
    pub fn write_yaneuraou_db2016(
        &self,
        w: &mut impl Write,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<(), BookError> {
        let ordered = self.ordered_entries(options.on_duplicate)?;

        writeln!(w, "{HEADER}")?;
        if options.emit_noe {
            writeln!(w, "# NOE:{}", ordered.len())?;
        }

        for entry in ordered {
            write_entry(w, entry, options)?;
        }
        Ok(())
    }

    /// 便宜的な `String` 版。内部で [`Self::write_yaneuraou_db2016`] を呼ぶ薄い wrapper。
    pub fn to_yaneuraou_db2016_string(
        &self,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<String, BookError> {
        let mut buffer = Vec::new();
        self.write_yaneuraou_db2016(&mut buffer, options)?;
        String::from_utf8(buffer).map_err(|err| {
            BookError::InvalidData(format!("yaneuraou writer produced non-utf8: {err}"))
        })
    }

    /// normalized SFEN 昇順かつ重複解決済みの entry 列を返す。
    fn ordered_entries(
        &self,
        on_duplicate: OnDuplicate,
    ) -> Result<Vec<&BookDatabaseEntry>, BookError> {
        let mut sorted: BTreeMap<&str, &BookDatabaseEntry> = BTreeMap::new();
        for entry in self.entries() {
            let key = entry.position().sfen();
            match on_duplicate {
                OnDuplicate::Error => {
                    if sorted.insert(key, entry).is_some() {
                        return Err(BookError::InvalidData(format!(
                            "duplicate normalized sfen in book database: {key}"
                        )));
                    }
                }
                OnDuplicate::KeepLast => {
                    sorted.insert(key, entry);
                }
            }
        }
        Ok(sorted.into_values().collect())
    }
}

fn write_entry(
    w: &mut impl Write,
    entry: &BookDatabaseEntry,
    options: &YaneuraOuDb2016WriteOptions,
) -> Result<(), BookError> {
    writeln!(w, "sfen {}", position_sfen(entry, options.ply))?;

    // lossless profile では entry コメントを sfen 行直後に出す（read 側は moves が空の間
    // のコメントを entry コメントとして取り込む）。
    if options.columns == ColumnSet::Lossless
        && let Some(comment) = entry_comment(entry)
    {
        write_comment_lines(w, comment)?;
    }

    let order = move_order(entry.candidates(), options.move_order);
    for &index in &order {
        write_move(w, &entry.candidates()[index], options)?;
    }
    Ok(())
}

/// position 行に出力する SFEN（末尾 ply を `PlyMode` に従って付与）。
fn position_sfen(entry: &BookDatabaseEntry, ply_mode: PlyMode) -> String {
    // normalized SFEN は `{board} {color} {hand} 1` の 4 フィールド。末尾を差し替える。
    let normalized = entry.position().sfen();
    let base = normalized.rsplit_once(' ').map_or(normalized, |(base, _)| base).to_string();

    let ply = match ply_mode {
        PlyMode::Preserve => match entry.position().original_ply() {
            Some(0) => None,
            Some(n) => Some(n),
            None => Some(1),
        },
        PlyMode::Fixed(n) => Some(n),
        PlyMode::Omit => None,
    };

    match ply {
        Some(n) => format!("{base} {n}"),
        None => base,
    }
}

fn move_order(candidates: &[BookCandidate], order: MoveOrder) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..candidates.len()).collect();
    if order == MoveOrder::ScoreDescending {
        // stable sort: score 降順、`None` は末尾、同 score は元順維持。
        indices.sort_by(|&a, &b| {
            score_key(candidates[b].score()).cmp(&score_key(candidates[a].score()))
        });
    }
    indices
}

/// 評価値降順 sort 用のキー。`None` は最小（= 末尾）に落とす。
fn score_key(score: Option<i32>) -> i64 {
    score.map_or(i64::MIN, i64::from)
}

fn write_move(
    w: &mut impl Write,
    candidate: &BookCandidate,
    options: &YaneuraOuDb2016WriteOptions,
) -> Result<(), BookError> {
    let mv = candidate.mv();
    if !mv.is_normal() {
        return Err(BookError::InvalidData(format!(
            "invalid yaneuraou book move: 0x{:04x}",
            mv.raw()
        )));
    }
    let move_str = mv.to_usi();
    match options.columns {
        ColumnSet::PetaShock => {
            // `move none value depth` の 4 列固定。
            writeln!(
                w,
                "{move_str} none {} {}",
                optional_field(candidate.score().map(i64::from)),
                optional_field(candidate.depth().map(i64::from)),
            )?;
        }
        ColumnSet::Lossless => {
            let ponder = match candidate.ponder() {
                Some(mv) if mv != Move::MOVE_NONE => mv.to_usi(),
                _ => "none".to_string(),
            };
            if let Some(mv) = candidate.ponder()
                && mv != Move::MOVE_NONE
                && !mv.is_normal()
            {
                return Err(BookError::InvalidData(format!(
                    "invalid yaneuraou ponder move: 0x{:04x}",
                    mv.raw()
                )));
            }
            write!(
                w,
                "{move_str} {ponder} {} {}",
                optional_field(candidate.score().map(i64::from)),
                optional_field(candidate.depth().map(i64::from)),
            )?;
            if let Some(count) = candidate.count() {
                write!(w, " {count}")?;
            }
            writeln!(w)?;

            if let Some(comment) = candidate_comment(candidate) {
                write_comment_lines(w, comment)?;
            }
        }
    }
    Ok(())
}

/// `Option<i64>` を DB2016 のフィールド表現にする（`None` は `none`）。
fn optional_field(value: Option<i64>) -> String {
    value.map_or_else(|| "none".to_string(), |v| v.to_string())
}

/// entry（position 行）に付くコメント。空文字列は `None` にする。
fn entry_comment(entry: &BookDatabaseEntry) -> Option<&str> {
    entry.metadata().yaneuraou().map(|m| m.comment()).filter(|c| !c.is_empty())
}

/// candidate（指し手行）に付くコメント。空文字列は `None` にする。
fn candidate_comment(candidate: &BookCandidate) -> Option<&str> {
    candidate.metadata().yaneuraou().map(|m| m.comment()).filter(|c| !c.is_empty())
}

/// 複数行コメントを 1 行ずつ `# ...` 行へ分解して出力する。
fn write_comment_lines(w: &mut impl Write, comment: &str) -> Result<(), io::Error> {
    for line in comment.split('\n') {
        writeln!(w, "# {line}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{self, InitialPosition};
    use crate::book::{
        BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata, BookPosition, YaneuraOuBook,
        YaneuraOuBookDiagnostics,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    // read 側テストで使われている有効な SFEN。normalized 形（末尾 " 1"）は文字列順で
    // A < B < C（先頭文字 '+' < 'l'、A と B は 2 文字目で分岐）ではないので、writer の
    // 並べ替えを検証できる。
    const A: &str = "+B3g3l/5rgk1/pB+P1ppn1p/n4spp1/1G1SP3P/K2P5/1+pS3P2/P2+l+r4/LNP6 b SNL2Pg2p";
    const B: &str = "+P1kg3nl/1ps2b3/+P3p3p/2pgsr1p1/s2p1pP2/2P1P1pR1/1SNG1P2P/1KG6/7NL w N2LPb2p";
    const C: &str = "lnB4nl/4k1g2/p3pps2/1G5rp/1pPPsb3/3p2P1P/PPS1PP3/2G1KSGP1/LN5NL w R2Pp";

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_path() -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("rsshogi-book-writer-{}-{stamp}-{seq}.db", std::process::id()))
    }

    fn candidate(usi: &str, score: Option<i32>, depth: Option<u32>) -> BookCandidate {
        BookCandidate::new(
            Move::from_usi(usi).expect("move"),
            None,
            score,
            depth,
            None,
            None,
            BookMoveMetadata::new(),
        )
    }

    fn entry(sfen: &str, ply: Option<u32>, candidates: Vec<BookCandidate>) -> BookDatabaseEntry {
        let position = BookPosition::from_sfen(sfen, ply).expect("position");
        BookDatabaseEntry::new(position, candidates, BookEntryMetadata::new())
    }

    fn reopen(text: &str) -> (YaneuraOuBook, PathBuf) {
        let path = temp_path();
        fs::write(&path, text).expect("write");
        let book = YaneuraOuBook::open(&path).expect("open");
        (book, path)
    }

    #[test]
    fn test_writer_output_is_sorted_and_complete() {
        board::init();
        // entry を意図的に降順（C, B, A）で投入し、writer が昇順へ並べ替えることを確認。
        let database = BookDatabase::try_from_entries(vec![
            entry(C, Some(64), vec![candidate("4e7h+", Some(540), Some(38))]),
            entry(B, Some(78), vec![candidate("4e4f", Some(120), Some(40))]),
            entry(A, Some(1), vec![candidate("9f9e", Some(0), Some(32))]),
        ])
        .unwrap();

        let text =
            database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new()).unwrap();
        assert!(text.starts_with("#YANEURAOU-DB2016 1.00\n"));

        let (book, path) = reopen(&text);
        assert!(matches!(
            book.diagnostics(),
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_writer_round_trip_preserves_positions_and_moves() {
        board::init();
        let fixture = format!(
            "#YANEURAOU-DB2016 1.00\n\
             sfen {A} 1\n\
             9f9e 8g7g 0 32 1 #main move\n\
             #move note\n\
             sfen {B} 78\n\
             #entry note\n\
             4e4f 9c9d 120 40 2\n\
             6e6f 6g6h -32 32 0\n\
             sfen {C} 64\n\
             4e7h+ none 540 38 1\n",
        );
        let (book, fixture_path) = reopen(&fixture);
        let entries: Vec<_> = book
            .iter_entries()
            .unwrap()
            .map(|entry| BookDatabaseEntry::from_yaneuraou(&entry.unwrap()).unwrap())
            .collect();
        let database = BookDatabase::try_from_entries(entries).unwrap();

        let text =
            database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new()).unwrap();
        let (reread, out_path) = reopen(&text);

        assert!(matches!(
            reread.diagnostics(),
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. }
        ));

        // 比較は BookDatabase が採用する normalized SFEN を key とする集合一致。
        // 各 position の指し手は採用した move sort 下でリスト一致（count/comment も Lossless で一致）。
        for source in database.entries() {
            let roundtrip = reread.lookup_sfen(source.position().sfen()).unwrap().unwrap();
            assert_eq!(source.position().sfen(), roundtrip.sfen());

            // database 側は評価値降順 sort 後の並びと比較する。
            let order = move_order(source.candidates(), MoveOrder::ScoreDescending);
            assert_eq!(order.len(), roundtrip.moves().len());
            for (&idx, b) in order.iter().zip(roundtrip.moves()) {
                let a = &source.candidates()[idx];
                assert_eq!(a.mv().to_usi(), b.mv().to_usi());
                assert_eq!(a.score(), b.score());
                assert_eq!(a.depth(), b.depth());
                assert_eq!(a.count(), b.count());
                assert_eq!(candidate_comment(a).unwrap_or(""), b.comment());
            }
        }
        fs::remove_file(fixture_path).unwrap();
        fs::remove_file(out_path).unwrap();
    }

    #[test]
    fn test_writer_score_descending_is_deterministic_with_none_last_and_stable_ties() {
        board::init();
        // 同 score(50) が 2 つ・None が 1 つ・最大 120。期待出力順: 120, 50(2g2f), 50(8h2b+), none。
        let database = BookDatabase::try_from_entries(vec![entry(
            InitialPosition::Standard.to_sfen(),
            Some(1),
            vec![
                candidate("2g2f", Some(50), Some(10)),
                candidate("8h2b+", Some(50), Some(20)),
                candidate("3i4h", None, Some(5)),
                candidate("7g7f", Some(120), Some(8)),
            ],
        )])
        .unwrap();

        let text =
            database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new()).unwrap();
        let moves: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("sfen"))
            .map(|line| line.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(moves, vec!["7g7f", "2g2f", "8h2b+", "3i4h"]);
    }

    #[test]
    fn test_writer_preserve_move_order() {
        board::init();
        let database = BookDatabase::try_from_entries(vec![entry(
            InitialPosition::Standard.to_sfen(),
            Some(1),
            vec![
                candidate("3i4h", None, Some(5)),
                candidate("7g7f", Some(120), Some(8)),
                candidate("2g2f", Some(50), Some(10)),
            ],
        )])
        .unwrap();

        let text = database
            .to_yaneuraou_db2016_string(
                &YaneuraOuDb2016WriteOptions::new().with_preserved_move_order(),
            )
            .unwrap();
        let moves: Vec<&str> = text
            .lines()
            .filter(|line| !line.starts_with('#') && !line.starts_with("sfen"))
            .map(|line| line.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(moves, vec!["3i4h", "7g7f", "2g2f"]);
    }

    #[test]
    fn test_writer_rejects_none_main_move() {
        board::init();
        let position = BookPosition::from_sfen(InitialPosition::Standard.to_sfen(), Some(1))
            .expect("position");
        let database = BookDatabase::from_entries_unchecked(vec![BookDatabaseEntry::new(
            position,
            vec![BookCandidate::new(
                Move::MOVE_NONE,
                None,
                Some(1),
                Some(1),
                None,
                None,
                BookMoveMetadata::new(),
            )],
            BookEntryMetadata::new(),
        )]);

        let err = database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new());
        assert!(
            matches!(err, Err(BookError::InvalidData(msg)) if msg.contains("invalid yaneuraou book move"))
        );
    }

    #[test]
    fn test_writer_ply_modes() {
        board::init();
        let sfen = InitialPosition::Standard.to_sfen();
        let base: String = {
            let normalized = BookPosition::from_sfen(sfen, None).unwrap().sfen().to_string();
            normalized.rsplit_once(' ').unwrap().0.to_string()
        };

        // Preserve: Some(99) -> 99 / Some(0) -> 省略 / None -> 1
        let preserve = |ply: Option<u32>| {
            let database = BookDatabase::try_from_entries(vec![entry(
                sfen,
                ply,
                vec![candidate("7g7f", Some(1), Some(1))],
            )])
            .unwrap();
            database
                .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new())
                .unwrap()
                .lines()
                .find(|line| line.starts_with("sfen"))
                .unwrap()
                .to_string()
        };
        assert_eq!(preserve(Some(99)), format!("sfen {base} 99"));
        assert_eq!(preserve(Some(0)), format!("sfen {base}"));
        assert_eq!(preserve(None), format!("sfen {base} 1"));

        // Fixed / Omit
        let database = BookDatabase::try_from_entries(vec![entry(
            sfen,
            Some(7),
            vec![candidate("7g7f", Some(1), Some(1))],
        )])
        .unwrap();
        let fixed = database
            .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new().with_fixed_ply(42))
            .unwrap();
        assert!(fixed.contains(&format!("sfen {base} 42\n")));
        let omit = database
            .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new().with_omitted_ply())
            .unwrap();
        assert!(omit.contains(&format!("sfen {base}\n")));
    }

    #[test]
    fn test_writer_duplicate_normalized_sfen_error_and_keep_last() {
        board::init();
        let sfen = InitialPosition::Standard.to_sfen();
        // BookDatabase は normalized sfen 一意を保証するため、writer の重複検出は
        // validate を経由しない from_entries_unchecked で同一 sfen 2 entry を作って検証する。
        let position = BookPosition::from_sfen(sfen, Some(1)).unwrap();
        let dup_db = BookDatabase::from_entries_unchecked(vec![
            BookDatabaseEntry::new(
                position.clone(),
                vec![candidate("7g7f", Some(1), Some(1))],
                BookEntryMetadata::new(),
            ),
            BookDatabaseEntry::new(
                position,
                vec![candidate("2g2f", Some(2), Some(2))],
                BookEntryMetadata::new(),
            ),
        ]);

        let err = dup_db.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new());
        assert!(
            matches!(err, Err(BookError::InvalidData(msg)) if msg.contains("duplicate normalized sfen"))
        );

        let keep_last = dup_db
            .to_yaneuraou_db2016_string(
                &YaneuraOuDb2016WriteOptions::new().with_keep_last_on_duplicate(),
            )
            .unwrap();
        // 後勝ちで 2g2f の entry が残る。
        let move_line = keep_last
            .lines()
            .find(|line| !line.starts_with('#') && !line.starts_with("sfen"))
            .unwrap();
        assert!(move_line.starts_with("2g2f"));
    }

    #[test]
    fn test_writer_peta_shock_profile_emits_four_columns() {
        board::init();
        let database = BookDatabase::try_from_entries(vec![entry(
            InitialPosition::Standard.to_sfen(),
            Some(1),
            vec![BookCandidate::new(
                Move::from_usi("7g7f").unwrap(),
                Move::from_usi("3c3d"),
                Some(120),
                Some(40),
                None,
                Some(7),
                BookMoveMetadata::from_yaneuraou("note".to_string()),
            )],
        )])
        .unwrap();

        let text = database
            .to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::peta_shock())
            .unwrap();
        let move_line =
            text.lines().find(|line| !line.starts_with('#') && !line.starts_with("sfen")).unwrap();
        // ponder は常に none、count/comment は落ちる、4 列。
        assert_eq!(move_line, "7g7f none 120 40");
        assert!(!text.contains("# note"));
    }

    #[test]
    fn test_writer_multiline_comment_is_split_into_hash_lines() {
        board::init();
        let database = BookDatabase::try_from_entries(vec![entry(
            InitialPosition::Standard.to_sfen(),
            Some(1),
            vec![BookCandidate::new(
                Move::from_usi("7g7f").unwrap(),
                None,
                Some(1),
                Some(1),
                None,
                None,
                BookMoveMetadata::from_yaneuraou("line one\nline two".to_string()),
            )],
        )])
        .unwrap();

        let text =
            database.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::new()).unwrap();
        assert!(text.contains("# line one\n"));
        assert!(text.contains("# line two\n"));
    }
}
