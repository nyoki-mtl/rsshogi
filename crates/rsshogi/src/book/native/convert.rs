use std::collections::BTreeMap;
use std::path::Path;

use crate::board::Position;
use crate::book::{BookDatabase, BookDatabaseEntry, BookError, SbkBook, YaneuraOuBook};

use super::{
    NativeBookBuilder, NativeBookEntry, NativeBookMetadata, NativeBookPosition, NativeBound,
    NativeCandidate, NativeScore,
};

const ISSUE_EXAMPLES: usize = 32;

/// v4 変換で保存不能なフィールドや局面を捨てるかを指定する。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeBookImportOptions {
    /// 明示した欠落の集計とともに変換を続ける。
    pub allow_lossy: bool,
}

/// 変換で保存できなかった入力位置とフィールド。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookImportIssue {
    /// 入力 iterator 内の 0 始まりの位置。文書メタデータでは 0。
    pub input_index: u64,
    /// 対象の SFEN。文書メタデータでは未設定。
    pub sfen: Option<String>,
    /// 保存できないフィールド名。
    pub field: String,
    /// 保存できない理由。
    pub reason: String,
}

/// v4 変換で保持した件数と欠落の集計。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeBookImportReport {
    /// 出力した局面数。
    pub positions: u64,
    /// 出力した候補手数。
    pub candidates: u64,
    /// 表現不能のため除外した局面数。
    pub skipped_positions: u64,
    /// 保存しなかったフィールドごとの出現件数。
    pub dropped_fields: BTreeMap<String, u64>,
    /// 先頭 32 件までの入力位置付き欠落例。
    pub issues: Vec<NativeBookImportIssue>,
    /// 例から省略した欠落件数。集計からは省略しない。
    pub omitted_issues: u64,
}

struct Import {
    builder: NativeBookBuilder,
    options: NativeBookImportOptions,
    report: NativeBookImportReport,
}

impl Import {
    fn new(
        path: &Path,
        metadata: NativeBookMetadata,
        options: NativeBookImportOptions,
    ) -> Result<Self, BookError> {
        Ok(Self {
            builder: NativeBookBuilder::new(path, metadata)?,
            options,
            report: NativeBookImportReport::default(),
        })
    }

    fn loss(&mut self, input_index: u64, sfen: Option<&str>, field: &str, reason: &str) {
        *self.report.dropped_fields.entry(field.to_owned()).or_default() += 1;
        if self.report.issues.len() < ISSUE_EXAMPLES {
            self.report.issues.push(NativeBookImportIssue {
                input_index,
                sfen: sfen.map(ToOwned::to_owned),
                field: field.into(),
                reason: reason.into(),
            });
        } else {
            self.report.omitted_issues += 1;
        }
    }

    fn position(&mut self, input_index: u64, sfen: &str) -> Result<Option<Position>, BookError> {
        let decoded =
            Position::from_sfen(sfen).map_err(|error| error.to_string()).and_then(|position| {
                position.try_to_packed_sfen().map_err(|error| error.to_string())?;
                Ok(position)
            });
        match decoded {
            Ok(position) => Ok(Some(position)),
            Err(reason) => {
                self.loss(
                    input_index,
                    Some(sfen),
                    "position",
                    &format!("position cannot be represented by native v4: {reason}"),
                );
                self.report.skipped_positions += 1;
                Ok(None)
            }
        }
    }

    fn entry(
        &mut self,
        input_index: u64,
        entry: &BookDatabaseEntry,
        position: Position,
    ) -> Result<(), BookError> {
        let sfen = entry.position().sfen();
        if entry.candidates().len() > usize::from(u16::MAX) {
            return Err(BookError::InvalidData(format!(
                "native import input {input_index} has too many candidates: {sfen}"
            )));
        }
        if entry.position().original_ply().is_some() {
            self.loss(
                input_index,
                Some(sfen),
                "position.original_ply",
                "native position identity excludes source move numbers",
            );
        }
        let mut comment = None;
        if let Some(metadata) = entry.metadata().yaneuraou() {
            self.loss(
                input_index,
                Some(sfen),
                "yaneuraou.min_ply",
                "native v4 has no source minimum-ply field",
            );
            comment = Some(metadata.comment().to_owned());
        }
        if let Some(metadata) = entry.metadata().sbk() {
            if metadata.state_id().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.state_id",
                    "native v4 does not preserve source graph identifiers",
                );
            }
            self.loss(
                input_index,
                Some(sfen),
                "sbk.state_index",
                "native v4 does not preserve source array ordinals",
            );
            for (field, value) in [
                ("sbk.games", metadata.games()),
                ("sbk.won_black", metadata.won_black()),
                ("sbk.won_white", metadata.won_white()),
            ] {
                if value.is_some() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        field,
                        "position-level game statistics are not candidate counts",
                    );
                }
            }
            comment = metadata.comment().map(ToOwned::to_owned);
            for evaluation in metadata.evals() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.state_eval",
                    "state-level evaluations cannot be duplicated into candidate scores",
                );
                for (field, present) in [
                    ("sbk.state_eval.value", evaluation.evaluation_value().is_some()),
                    ("sbk.state_eval.depth", evaluation.depth().is_some()),
                    ("sbk.state_eval.sel_depth", evaluation.sel_depth().is_some()),
                    ("sbk.state_eval.nodes", evaluation.nodes().is_some()),
                    ("sbk.state_eval.pv", evaluation.variation().is_some()),
                    ("sbk.state_eval.engine_name", evaluation.engine_name().is_some()),
                ] {
                    if present {
                        self.loss(
                            input_index,
                            Some(sfen),
                            field,
                            "native v4 has no state-level engine analysis fields",
                        );
                    }
                }
            }
        }
        let mut candidates = Vec::with_capacity(entry.candidates().len());
        let mut move_comments = BTreeMap::new();
        for (rank, candidate) in entry.candidates().iter().enumerate() {
            if candidate.count().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "candidate.count",
                    &format!(
                        "native does not store candidate counts; move_raw={}",
                        candidate.mv().raw()
                    ),
                );
            }
            if candidate.ponder().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "candidate.ponder",
                    &format!(
                        "native v4 does not store ponder moves; move_raw={}",
                        candidate.mv().raw()
                    ),
                );
            }
            if candidate.weight().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "candidate.weight",
                    &format!(
                        "weights are not candidate counts or evaluations; move_raw={}",
                        candidate.mv().raw()
                    ),
                );
            }
            let mut score = candidate.score();
            if let Some(metadata) = candidate.metadata().sbk() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.move_raw",
                    &format!(
                        "native v4 stores its own validated move encoding; move_raw={}",
                        candidate.mv().raw()
                    ),
                );
                if metadata.evaluation().is_some() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "sbk.move_evaluation",
                        &format!("SBK move evaluation is an annotation enum, not a centipawn score; move_raw={}", candidate.mv().raw()),
                    );
                    if score == metadata.evaluation() {
                        score = None;
                    }
                }
                if metadata.weight().is_some() && candidate.weight().is_none() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "candidate.weight",
                        &format!(
                            "weights are not candidate counts or evaluations; move_raw={}",
                            candidate.mv().raw()
                        ),
                    );
                }
                if metadata.next_state_id().is_some() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "sbk.next_state_id",
                        &format!(
                            "native v4 does not preserve source graph links; move_raw={}",
                            candidate.mv().raw()
                        ),
                    );
                }
            }
            if let Some(metadata) = candidate.metadata().yaneuraou()
                && !metadata.comment().is_empty()
            {
                move_comments.insert(candidate.mv().raw(), metadata.comment().to_owned());
            }
            candidates.push(NativeCandidate {
                mv: candidate.mv(),
                score: score.map_or(NativeScore::Unknown, |value| NativeScore::Cp {
                    value,
                    bound: NativeBound::Exact,
                }),
                depth: candidate.depth(),
                display_order: rank as u16,
            });
        }
        let native = NativeBookEntry::from_position(&position, candidates).map_err(|error| {
            BookError::InvalidData(format!(
                "native import input {input_index}, SFEN {sfen}: {error}"
            ))
        })?;
        let candidate_count = native.candidates.len() as u64;
        self.builder.push(NativeBookPosition { entry: native, comment, move_comments })?;
        self.report.positions += 1;
        self.report.candidates += candidate_count;
        Ok(())
    }

    fn finish(self) -> Result<NativeBookImportReport, BookError> {
        if !self.options.allow_lossy && !self.report.dropped_fields.is_empty() {
            return Err(BookError::InvalidData(format!(
                "native import requires explicit allow_lossy: positions={}, candidates={}, skipped_positions={}, dropped_fields={:?}, examples={:?}, omitted_issues={}",
                self.report.positions,
                self.report.candidates,
                self.report.skipped_positions,
                self.report.dropped_fields,
                self.report.issues,
                self.report.omitted_issues,
            )));
        }
        self.builder.finish()?;
        Ok(self.report)
    }
}

impl BookDatabase {
    /// 所有 IR を v4 へ順次変換し、欠落を集計して新規ファイルへ書き出す。
    ///
    /// 評価整数は候補手を指す前の手番視点の cp として扱う。
    /// SBK 注釈から複製された整数は評価値として扱わない。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正な候補手、I/O エラー、既存保存先がある場合に返す。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let mut import = Import::new(path.as_ref(), NativeBookMetadata::default(), options)?;
        for (index, entry) in self.entries().iter().enumerate() {
            if let Some(position) = import.position(index as u64, entry.position().sfen())? {
                import.entry(index as u64, entry, position)?;
            }
        }
        import.finish()
    }
}

impl YaneuraOuBook {
    /// DB2016 の局面を 1 件ずつ v4 へ変換し、欠落を集計する。
    ///
    /// score 整数は手番視点の cp として扱い、独自の詰み距離符号化は推測しない。
    /// 入力エラーと I/O エラーは lossy 指定でも中断する。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正入力、I/O エラー、既存保存先がある場合に返す。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let metadata = NativeBookMetadata { depth_kind: 1, ..NativeBookMetadata::default() };
        let mut import = Import::new(path.as_ref(), metadata, options)?;
        for (index, entry) in self.iter_entries()?.enumerate() {
            let entry = entry?;
            if let Some(position) = import.position(index as u64, entry.sfen())? {
                let decoded = BookDatabase::entry_from_yaneuraou(&entry)?;
                import.entry(index as u64, &decoded, position)?;
            }
        }
        import.finish()
    }
}

impl SbkBook {
    /// SBK の局面を 1 件ずつ v4 へ変換し、状態評価や注釈を欠落として報告する。
    ///
    /// 文書 description と局面コメントを保持する。author を title へ転用しない。
    /// SBK リーダーが解決した局面だけを対象とし、未解決状態と重複も集計する。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正入力、I/O エラー、既存保存先がある場合に返す。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let metadata = NativeBookMetadata {
            title: None,
            description: self.description().map(ToOwned::to_owned),
            ..NativeBookMetadata::default()
        };
        let mut import = Import::new(path.as_ref(), metadata, options)?;
        if self.author().is_some() {
            import.loss(0, None, "sbk.author", "native v4 has no author field");
        }
        for _ in 0..self.diagnostics().duplicate_positions() {
            import.loss(
                0,
                None,
                "sbk.duplicate_position",
                "SBK reader already deduplicated this position; source target is unavailable",
            );
        }
        for _ in 0..self.diagnostics().unresolved_next_states() {
            import.loss(
                0,
                None,
                "sbk.unresolved_state",
                "SBK reader could not resolve this source state position",
            );
            import.report.skipped_positions += 1;
        }
        for (index, entry) in self.iter_entries().enumerate() {
            let entry = entry?;
            if let Some(position) = import.position(index as u64, entry.sfen())? {
                let decoded = BookDatabase::entry_from_sbk(&entry)?;
                import.entry(index as u64, &decoded, position)?;
            }
        }
        import.finish()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::board;
    use crate::book::{
        BookCandidate, BookEntryMetadata, BookMoveMetadata, BookPosition, NativeBook,
        SbkEntryMetadata, SbkEvalMetadata, SbkMoveMetadata, SbkWriteOptions,
    };
    use crate::types::Move;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            board::init();
            let path = super::super::temporary_path(
                &std::env::temp_dir().join("rsshogi-native-import"),
                "test",
            );
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            for name in
                ["source.db", "source.sbk", "output.book", "strict.book", "skip.book", "bad.book"]
            {
                let _ = fs::remove_file(self.path(name));
            }
            fs::remove_dir(&self.0).expect("conversion must clean temporary runs");
        }
    }

    fn candidate(usi: &str, score: Option<i32>, weight: Option<i32>) -> BookCandidate {
        BookCandidate::new(
            Move::from_usi(usi).unwrap(),
            None,
            score,
            Some(0),
            weight,
            None,
            BookMoveMetadata::new(),
        )
    }

    fn database(candidates: Vec<BookCandidate>, metadata: BookEntryMetadata) -> BookDatabase {
        BookDatabase::try_from_entries(vec![BookDatabaseEntry::new(
            BookPosition::from_position(&board::hirate_position(), None),
            candidates,
            metadata,
        )])
        .unwrap()
    }

    #[test]
    fn database_preserves_optional_values_comments_and_display_order() {
        let directory = TestDirectory::new();
        let first = BookCandidate::new(
            Move::from_usi("7g7f").unwrap(),
            None,
            None,
            None,
            None,
            None,
            BookMoveMetadata::from_yaneuraou("first\r\ncomment".into()),
        );
        let second = candidate("2g2f", Some(0), None);
        let database = database(
            vec![first, second],
            BookEntryMetadata::from_yaneuraou(1, "position\rcomment".into()),
        );
        let path = directory.path("output.book");
        let report = database
            .write_native_book(&path, NativeBookImportOptions { allow_lossy: true })
            .unwrap();
        assert_eq!(report.positions, 1);
        assert_eq!(report.candidates, 2);
        assert_eq!(report.dropped_fields.get("yaneuraou.min_ply"), Some(&1));
        let book = NativeBook::open(&path).unwrap();
        book.validate_full().unwrap();
        let record = book.record(&board::hirate_position()).unwrap().unwrap();
        assert_eq!(record.comment.as_deref(), Some("position\ncomment"));
        let first = record
            .entry
            .candidates
            .iter()
            .find(|candidate| candidate.mv.to_usi() == "7g7f")
            .unwrap();
        let second = record
            .entry
            .candidates
            .iter()
            .find(|candidate| candidate.mv.to_usi() == "2g2f")
            .unwrap();
        assert_eq!(first.score, NativeScore::Unknown);
        assert_eq!(first.depth, None);
        assert_eq!(first.display_order, 0);
        assert_eq!(second.score, NativeScore::Cp { value: 0, bound: NativeBound::Exact });
        assert_eq!(second.depth, Some(0));
        assert_eq!(second.display_order, 1);
        assert_eq!(
            record.move_comments.get(&first.mv.raw()).map(String::as_str),
            Some("first\ncomment")
        );
    }

    #[test]
    fn strict_weight_loss_leaves_no_output_and_lossy_never_turns_weight_into_score() {
        let directory = TestDirectory::new();
        let database = database(vec![candidate("7g7f", None, Some(123))], BookEntryMetadata::new());
        let strict = directory.path("strict.book");
        let error =
            database.write_native_book(&strict, NativeBookImportOptions::default()).unwrap_err();
        assert!(error.to_string().contains("candidate.weight"));
        assert!(!strict.exists());
        let path = directory.path("output.book");
        let report = database
            .write_native_book(&path, NativeBookImportOptions { allow_lossy: true })
            .unwrap();
        assert_eq!(report.dropped_fields.get("candidate.weight"), Some(&1));
        let entry =
            NativeBook::open(&path).unwrap().get(&board::hirate_position()).unwrap().unwrap();
        assert_eq!(entry.candidates[0].score, NativeScore::Unknown);
        assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    }

    #[test]
    fn candidate_counts_require_lossy_permission_including_zero() {
        for count in [0, u64::MAX] {
            let directory = TestDirectory::new();
            let source = BookCandidate::new(
                Move::from_usi("7g7f").unwrap(),
                None,
                Some(25),
                Some(16),
                None,
                Some(count),
                BookMoveMetadata::new(),
            );
            let database = database(vec![source], BookEntryMetadata::new());
            let strict = directory.path("strict.book");
            let error = database
                .write_native_book(&strict, NativeBookImportOptions::default())
                .unwrap_err();
            assert!(error.to_string().contains("candidate.count"));
            assert!(!strict.exists());

            let output = directory.path("output.book");
            let report = database
                .write_native_book(&output, NativeBookImportOptions { allow_lossy: true })
                .unwrap();
            assert_eq!(report.dropped_fields.get("candidate.count"), Some(&1));
            assert_eq!(report.issues.len(), 1);
            assert_eq!(report.issues[0].field, "candidate.count");
            assert_eq!(report.issues[0].input_index, 0);
            assert_eq!(
                report.issues[0].sfen.as_deref(),
                Some(board::hirate_position().to_sfen(None).as_str())
            );
            let book = NativeBook::open(&output).unwrap();
            book.validate_full().unwrap();
            let entry = book.get(&board::hirate_position()).unwrap().unwrap();
            assert_eq!(entry.candidates[0].depth, Some(16));
            assert_eq!(
                entry.candidates[0].score,
                NativeScore::Cp { value: 25, bound: NativeBound::Exact }
            );
        }
    }

    #[test]
    fn explicit_cp_is_preserved_for_white_without_sign_or_range_narrowing() {
        let directory = TestDirectory::new();
        let sfen = board::hirate_position().to_sfen(None).replace(" b ", " w ");
        let position = Position::from_sfen(&sfen).unwrap();
        let entry = BookDatabaseEntry::new(
            BookPosition::from_position(&position, None),
            vec![candidate("3c3d", Some(i32::MIN), None), candidate("8c8d", Some(i32::MAX), None)],
            BookEntryMetadata::new(),
        );
        let database = BookDatabase::try_from_entries(vec![entry]).unwrap();
        let path = directory.path("output.book");
        let report = database.write_native_book(&path, NativeBookImportOptions::default()).unwrap();
        assert!(report.dropped_fields.is_empty());
        let record = NativeBook::open(&path).unwrap().get(&position).unwrap().unwrap();
        let values: Vec<_> = record.candidates.iter().map(|candidate| candidate.score).collect();
        assert!(values.contains(&NativeScore::Cp { value: i32::MIN, bound: NativeBound::Exact }));
        assert!(values.contains(&NativeScore::Cp { value: i32::MAX, bound: NativeBound::Exact }));
    }

    #[test]
    fn representability_reports_sfen_index_and_bounded_examples() {
        let directory = TestDirectory::new();
        let path = directory.path("skip.book");
        let mut import = Import::new(
            &path,
            NativeBookMetadata::default(),
            NativeBookImportOptions { allow_lossy: true },
        )
        .unwrap();
        for index in 0..40 {
            assert!(import.position(index, "invalid sfen").unwrap().is_none());
        }
        let report = import.finish().unwrap();
        assert_eq!(report.skipped_positions, 40);
        assert_eq!(report.positions, 0);
        assert_eq!(report.dropped_fields.get("position"), Some(&40));
        assert_eq!(report.issues.len(), 32);
        assert_eq!(report.omitted_issues, 8);
        assert_eq!(report.issues[31].input_index, 31);
        assert_eq!(report.issues[0].sfen.as_deref(), Some("invalid sfen"));
        let strict = directory.path("strict.book");
        let mut import =
            Import::new(&strict, NativeBookMetadata::default(), NativeBookImportOptions::default())
                .unwrap();
        assert!(import.position(7, "9/9/9/9/9/9/9/9/4K4 b - 1").unwrap().is_none());
        let error = import.finish().unwrap_err().to_string();
        assert!(error.contains("input_index: 7"));
        assert!(error.contains("9/9/9/9/9/9/9/9/4K4"));
        assert!(!strict.exists());
    }

    #[test]
    fn yaneuraou_stream_conversion_and_source_io_errors() {
        let directory = TestDirectory::new();
        let source = directory.path("source.db");
        let sfen = board::hirate_position().to_sfen(None);
        fs::write(&source, format!("#YANEURAOU-DB2016 1.00\nsfen {sfen}\n7g7f none 0 0 9\n"))
            .unwrap();
        let book = YaneuraOuBook::open(&source).unwrap();
        let path = directory.path("output.book");
        let report =
            book.write_native_book(&path, NativeBookImportOptions { allow_lossy: true }).unwrap();
        assert_eq!(report.positions, 1);
        assert_eq!(report.dropped_fields.get("candidate.count"), Some(&1));
        assert_eq!(NativeBook::open(&path).unwrap().metadata().unwrap().depth_kind, 1);
        let entry =
            NativeBook::open(&path).unwrap().get(&board::hirate_position()).unwrap().unwrap();
        assert_eq!(
            entry.candidates[0].score,
            NativeScore::Cp { value: 0, bound: NativeBound::Exact }
        );
        fs::remove_file(&source).unwrap();
        let bad = directory.path("bad.book");
        assert!(matches!(
            book.write_native_book(&bad, NativeBookImportOptions { allow_lossy: true }),
            Err(BookError::Io(_))
        ));
        assert!(!bad.exists());
    }

    #[test]
    fn sbk_state_analysis_and_move_annotations_do_not_become_cp() {
        let directory = TestDirectory::new();
        let mv = Move::from_usi("7g7f").unwrap();
        let source_candidate = BookCandidate::new(
            mv,
            None,
            Some(1),
            None,
            Some(77),
            None,
            BookMoveMetadata::from_sbk(SbkMoveMetadata::new(
                0x0100_7677,
                Some(1),
                Some(77),
                Some(-1),
            )),
        );
        let metadata = BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
            Some(9),
            0,
            Some(55),
            None,
            None,
            Some("state comment".into()),
            vec![SbkEvalMetadata::new(
                Some(900),
                Some(30),
                None,
                Some(10),
                Some("7g7f".into()),
                Some("engine".into()),
            )],
        ));
        let database = database(vec![source_candidate], metadata);
        let source = directory.path("source.sbk");
        fs::write(&source, database.to_sbk_bytes(&SbkWriteOptions::new()).unwrap()).unwrap();
        let book = SbkBook::open(&source).unwrap();
        let strict = directory.path("strict.book");
        assert!(book.write_native_book(&strict, NativeBookImportOptions::default()).is_err());
        assert!(!strict.exists());
        let path = directory.path("output.book");
        let report =
            book.write_native_book(&path, NativeBookImportOptions { allow_lossy: true }).unwrap();
        assert_eq!(report.dropped_fields.get("sbk.state_eval"), Some(&1));
        assert_eq!(report.dropped_fields.get("sbk.state_eval.pv"), Some(&1));
        assert_eq!(report.dropped_fields.get("sbk.move_evaluation"), Some(&1));
        let book = NativeBook::open(&path).unwrap();
        book.validate_full().unwrap();
        let record = book.record(&board::hirate_position()).unwrap().unwrap();
        assert_eq!(record.comment.as_deref(), Some("state comment"));
        assert_eq!(record.entry.candidates[0].score, NativeScore::Unknown);
        assert_eq!(record.entry.candidates[0].depth, None);
    }
}
