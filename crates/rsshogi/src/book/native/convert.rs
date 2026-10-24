use std::collections::{BTreeMap, HashSet};
use std::fmt::Display;
use std::path::Path;

use crate::board::Position;
use crate::book::{BookDatabase, BookDatabaseEntry, BookError, SbkBook, YaneuraOuBook, YbbBook};
use crate::types::Move;

use super::{
    DEPTH_KIND_UNSPECIFIED, NativeBookBuilder, NativeBookEntry, NativeBookMetadata,
    NativeBookPosition, NativeBound, NativeCandidate, NativeScore,
};

const ISSUE_EXAMPLES: usize = 32;

/// v4 変換のメタデータ、欠落許可、構築予算。
///
/// 既定の深さ種別は [`super::DEPTH_KIND_UNSPECIFIED`] とする。
/// 不正入力は欠落許可や局面スキップの設定によらず拒否する。
/// 未許可の欠落は最初の検出時に中断し、後続入力は走査しない。
/// 入力検査と設定の不正は [`BookError::InvalidData`]、保存先の競合は
/// [`BookError::Io`] の [`std::io::ErrorKind::AlreadyExists`] で区別する。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookImportOptions {
    metadata: NativeBookMetadata,
    allow_lossy: bool,
    skip_unencodable_positions: bool,
    memory_budget_bytes: Option<usize>,
    block_positions: Option<u32>,
}

impl NativeBookImportOptions {
    /// 欠落と局面スキップを許可しない既定設定を返す。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            metadata: NativeBookMetadata {
                title: None,
                description: None,
                depth_kind: DEPTH_KIND_UNSPECIFIED,
            },
            allow_lossy: false,
            skip_unencodable_positions: false,
            memory_budget_bytes: None,
            block_positions: None,
        }
    }

    /// 出力メタデータを置き換える。深さ種別も指定値に置き換わる。
    ///
    /// SBK の説明は `None` なら継承し、`Some(String::new())` なら削除する。
    #[must_use]
    pub fn with_metadata(mut self, metadata: NativeBookMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// 出力の深さ種別を指定する。未知の値もそのまま保持する。
    #[must_use]
    pub const fn with_depth_kind(mut self, depth_kind: u32) -> Self {
        self.metadata.depth_kind = depth_kind;
        self
    }

    /// フィールド破棄と SBK reader の未解決状態の欠落を許可する。
    #[must_use]
    pub const fn with_allow_lossy(mut self, allow_lossy: bool) -> Self {
        self.allow_lossy = allow_lossy;
        self
    }

    /// SFEN と候補手の検査に成功した表現不能局面の除外を許可する。
    ///
    /// 除外行は `position` の欠落だけを記録し、個別フィールドは数えない。
    #[must_use]
    pub const fn with_skip_unencodable_positions(mut self, skip: bool) -> Self {
        self.skip_unencodable_positions = skip;
        self
    }

    /// builder の予算をバイト単位で指定する。変換開始時に 1 MiB 以上か検査する。
    #[must_use]
    pub const fn with_memory_budget(mut self, bytes: usize) -> Self {
        self.memory_budget_bytes = Some(bytes);
        self
    }

    /// ブロックあたりの局面数を指定する。変換開始時に `1..=16384` か検査する。
    #[must_use]
    pub const fn with_block_positions(mut self, block_positions: u32) -> Self {
        self.block_positions = Some(block_positions);
        self
    }
}

impl Default for NativeBookImportOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// 変換で保存できなかった入力位置とフィールド。
///
/// `field` は機械処理用の項目名、`reason` は表示用の説明とする。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookImportIssue {
    /// 入力 iterator 内の 0 始まりの位置。文書メタデータや SBK 診断では 0。
    pub input_index: u64,
    /// 対象の SFEN。文書メタデータや SBK 診断では未設定。
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
    /// 表現不能局面と SBK reader の未解決状態の合計数。
    pub skipped_positions: u64,
    /// 保存しなかったフィールドごとの出現件数。
    /// 既存キーと集計単位は保持する。未知のキーも表示または保持して扱う。
    pub dropped_fields: BTreeMap<String, u64>,
    /// 先頭 32 件までの入力位置付き欠落例。
    pub issues: Vec<NativeBookImportIssue>,
    /// 例から省略した欠落件数。集計からは省略しない。
    pub omitted_issues: u64,
}

struct Import<'a> {
    builder: NativeBookBuilder,
    options: &'a NativeBookImportOptions,
    report: NativeBookImportReport,
}

impl<'a> Import<'a> {
    fn new(
        path: &Path,
        metadata: NativeBookMetadata,
        options: &'a NativeBookImportOptions,
    ) -> Result<Self, BookError> {
        let mut builder = NativeBookBuilder::new(path, metadata)?;
        if let Some(bytes) = options.memory_budget_bytes {
            builder = builder.with_memory_budget(bytes)?;
        }
        if let Some(count) = options.block_positions {
            builder = builder.with_block_positions(count)?;
        }
        Ok(Self { builder, options, report: NativeBookImportReport::default() })
    }

    fn loss(
        &mut self,
        input_index: u64,
        sfen: Option<&str>,
        field: &str,
        reason: impl Display,
    ) -> Result<(), BookError> {
        if !self.options.allow_lossy {
            return Err(input_error(
                input_index,
                sfen,
                BookError::InvalidData(format!(
                    "native import requires explicit allow_lossy: field={field}: {reason}"
                )),
            ));
        }
        self.record_loss(input_index, sfen, field, reason);
        Ok(())
    }

    fn record_loss(
        &mut self,
        input_index: u64,
        sfen: Option<&str>,
        field: &str,
        reason: impl Display,
    ) {
        if let Some(count) = self.report.dropped_fields.get_mut(field) {
            *count += 1;
        } else {
            self.report.dropped_fields.insert(field.to_owned(), 1);
        }
        if self.report.issues.len() < ISSUE_EXAMPLES {
            self.report.issues.push(NativeBookImportIssue {
                input_index,
                sfen: sfen.map(ToOwned::to_owned),
                field: field.into(),
                reason: reason.to_string(),
            });
        } else {
            self.report.omitted_issues += 1;
        }
    }

    fn position(
        &mut self,
        input_index: u64,
        sfen: &str,
        moves: impl ExactSizeIterator<Item = Move>,
    ) -> Result<Option<[u8; 32]>, BookError> {
        let position = Position::from_sfen(sfen).map_err(|error| {
            BookError::InvalidData(format!(
                "native import input {input_index}, SFEN {sfen}: {error}"
            ))
        })?;
        validate_moves(&position, moves)
            .map_err(|error| input_error(input_index, Some(sfen), error))?;
        match position.try_to_packed_sfen() {
            Ok(packed) => Ok(Some(*packed.as_bytes())),
            Err(reason) if self.options.skip_unencodable_positions => {
                self.record_loss(
                    input_index,
                    Some(sfen),
                    "position",
                    format_args!("position cannot be represented by native v4: {reason}"),
                );
                self.report.skipped_positions += 1;
                Ok(None)
            }
            Err(reason) => Err(BookError::InvalidData(format!(
                "native import input {input_index}, SFEN {sfen}: {reason}"
            ))),
        }
    }

    fn entry(
        &mut self,
        input_index: u64,
        entry: &BookDatabaseEntry,
        key: [u8; 32],
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
            )?;
        }
        let mut comment = None;
        if let Some(metadata) = entry.metadata().yaneuraou() {
            if metadata.min_ply() != entry.position().original_ply().unwrap_or(0) {
                self.loss(
                    input_index,
                    Some(sfen),
                    "yaneuraou.min_ply",
                    "native v4 has no source minimum-ply field",
                )?;
            }
            comment = Some(metadata.comment().to_owned());
        }
        if let Some(metadata) = entry.metadata().sbk() {
            if metadata.state_id().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.state_id",
                    "native v4 does not preserve source graph identifiers",
                )?;
            }
            self.loss(
                input_index,
                Some(sfen),
                "sbk.state_index",
                "native v4 does not preserve source array ordinals",
            )?;
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
                    )?;
                }
            }
            comment = metadata.comment().map(ToOwned::to_owned);
            for evaluation in metadata.evals() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.state_eval",
                    "state-level evaluations cannot be duplicated into candidate scores",
                )?;
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
                        )?;
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
                    format_args!(
                        "native does not store candidate counts; move_raw={}",
                        candidate.mv().raw()
                    ),
                )?;
            }
            if candidate.ponder().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "candidate.ponder",
                    format_args!(
                        "native v4 does not store ponder moves; move_raw={}",
                        candidate.mv().raw()
                    ),
                )?;
            }
            if candidate.weight().is_some() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "candidate.weight",
                    format_args!(
                        "weights are not candidate counts or evaluations; move_raw={}",
                        candidate.mv().raw()
                    ),
                )?;
            }
            let mut score = candidate.score();
            if let Some(metadata) = candidate.metadata().sbk() {
                self.loss(
                    input_index,
                    Some(sfen),
                    "sbk.move_raw",
                    format_args!(
                        "native v4 stores its own validated move encoding; move_raw={}",
                        candidate.mv().raw()
                    ),
                )?;
                if metadata.evaluation().is_some() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "sbk.move_evaluation",
                        format_args!("SBK move evaluation is an annotation enum, not a centipawn score; move_raw={}", candidate.mv().raw()),
                    )?;
                    if score == metadata.evaluation() {
                        score = None;
                    }
                }
                if metadata.weight().is_some() && candidate.weight().is_none() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "candidate.weight",
                        format_args!(
                            "weights are not candidate counts or evaluations; move_raw={}",
                            candidate.mv().raw()
                        ),
                    )?;
                }
                if metadata.next_state_id().is_some() {
                    self.loss(
                        input_index,
                        Some(sfen),
                        "sbk.next_state_id",
                        format_args!(
                            "native v4 does not preserve source graph links; move_raw={}",
                            candidate.mv().raw()
                        ),
                    )?;
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
        candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
        self.push(
            input_index,
            || sfen.to_owned(),
            NativeBookPosition {
                entry: NativeBookEntry { key, candidates },
                comment,
                move_comments,
            },
        )?;
        Ok(())
    }

    fn push(
        &mut self,
        input_index: u64,
        sfen: impl Fn() -> String,
        record: NativeBookPosition,
    ) -> Result<(), BookError> {
        let candidate_count = record.entry.candidates.len() as u64;
        self.builder.push_with_input_context(record, |error| {
            if matches!(error, BookError::InvalidData(_)) {
                input_error(input_index, Some(&sfen()), error)
            } else {
                error
            }
        })?;
        self.report.positions += 1;
        self.report.candidates += candidate_count;
        Ok(())
    }

    fn finish(self) -> Result<NativeBookImportReport, BookError> {
        self.builder.finish()?;
        Ok(self.report)
    }
}

fn input_error(input_index: u64, sfen: Option<&str>, error: BookError) -> BookError {
    match error {
        BookError::InvalidData(reason) => BookError::InvalidData(match sfen {
            Some(sfen) => format!("native import input {input_index}, SFEN {sfen}: {reason}"),
            None => format!("native import input {input_index}: {reason}"),
        }),
        error => error,
    }
}

fn validate_moves(
    position: &Position,
    moves: impl ExactSizeIterator<Item = Move>,
) -> Result<(), BookError> {
    if moves.len() > usize::from(u16::MAX) {
        return Err(BookError::InvalidData("too many candidates".into()));
    }
    let mut seen = HashSet::with_capacity(moves.len());
    for mv in moves {
        if !mv.is_normal() || !position.is_legal_move(mv) {
            return Err(BookError::InvalidData(format!(
                "illegal candidate: move_raw={}",
                mv.raw()
            )));
        }
        if !seen.insert(mv) {
            return Err(BookError::InvalidData(format!(
                "duplicate candidate: move_raw={}",
                mv.raw()
            )));
        }
    }
    Ok(())
}

impl BookDatabase {
    /// 所有 IR を v4 へ順次変換し、欠落を集計して新規ファイルへ書き出す。
    ///
    /// 評価整数は候補手を指す前の手番視点の cp として扱う。
    /// SBK 注釈から複製された整数は評価値として扱わない。
    /// 表現不能局面も候補手を検査してから、専用オプションに従って除外する。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正な候補手、I/O エラー、既存保存先がある場合に返す。
    /// 公開後の失敗は [`BookError::SaveUncertain`] を返し、出力が存在し得る。
    /// この場合は出力を再オープンして内容を確認する。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: &NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let mut import = Import::new(path.as_ref(), options.metadata.clone(), options)?;
        for (index, entry) in self.entries().iter().enumerate() {
            if let Some(key) = import.position(
                index as u64,
                entry.position().sfen(),
                entry.candidates().iter().map(|candidate| candidate.mv()),
            )? {
                import.entry(index as u64, entry, key)?;
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
    /// 深さ種別はオプションに従い、未指定時は 0 とする。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正入力、I/O エラー、既存保存先がある場合に返す。
    /// 公開後の失敗は [`BookError::SaveUncertain`] を返し、出力が存在し得る。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: &NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let mut import = Import::new(path.as_ref(), options.metadata.clone(), options)?;
        for (index, entry) in self.iter_entries()?.enumerate() {
            let entry = entry?;
            let index = index as u64;
            if let Some(key) = import.position(
                index,
                entry.sfen(),
                entry.moves().iter().map(|candidate| candidate.mv()),
            )? {
                if entry.original_ply().is_some() {
                    import.loss(
                        index,
                        Some(entry.sfen()),
                        "position.original_ply",
                        "native position identity excludes source move numbers",
                    )?;
                }
                let mut candidates = Vec::with_capacity(entry.moves().len());
                let mut move_comments = BTreeMap::new();
                for (rank, candidate) in entry.moves().iter().enumerate() {
                    if candidate.count().is_some() {
                        import.loss(
                            index,
                            Some(entry.sfen()),
                            "candidate.count",
                            format_args!(
                                "native does not store candidate counts; move_raw={}",
                                candidate.mv().raw()
                            ),
                        )?;
                    }
                    if candidate.ponder() != Move::MOVE_NONE {
                        import.loss(
                            index,
                            Some(entry.sfen()),
                            "candidate.ponder",
                            format_args!(
                                "native v4 does not store ponder moves; move_raw={}",
                                candidate.mv().raw()
                            ),
                        )?;
                    }
                    if !candidate.comment().is_empty() {
                        move_comments.insert(candidate.mv().raw(), candidate.comment().to_owned());
                    }
                    candidates.push(NativeCandidate {
                        mv: candidate.mv(),
                        score: candidate.score().map_or(NativeScore::Unknown, |value| {
                            NativeScore::Cp { value, bound: NativeBound::Exact }
                        }),
                        depth: candidate.depth(),
                        display_order: rank as u16,
                    });
                }
                candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
                import.push(
                    index,
                    || entry.sfen().to_owned(),
                    NativeBookPosition {
                        entry: NativeBookEntry { key, candidates },
                        comment: Some(entry.comment().to_owned()),
                        move_comments,
                    },
                )?;
            }
        }
        import.finish()
    }
}

impl YbbBook {
    /// YBB を逐次検査して v4 の新規ファイルへ変換する。
    ///
    /// キーの復号と正準形、候補手の合法性と一意性を検査する。
    /// 評価値は手番視点の cp とし、深さの未設定と 0 を区別する。
    /// 各行の手数破棄には `with_allow_lossy(true)` を必要とする。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正入力、重複局面、I/O エラー、既存保存先を拒否する。
    /// 公開後の失敗は [`BookError::SaveUncertain`] を返し、出力が存在し得る。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: &NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let mut import = Import::new(path.as_ref(), options.metadata.clone(), options)?;
        for (index, entry) in self.iter_entries().enumerate() {
            let entry = entry?;
            let index = index as u64;
            let mut native = NativeBookEntry {
                key: *entry.packed_sfen().as_bytes(),
                candidates: entry
                    .moves()
                    .iter()
                    .enumerate()
                    .map(|(rank, candidate)| NativeCandidate {
                        mv: candidate.mv(),
                        score: NativeScore::Cp {
                            value: i32::from(candidate.eval()),
                            bound: NativeBound::Exact,
                        },
                        depth: candidate.depth().map(u32::from),
                        display_order: rank as u16,
                    })
                    .collect(),
            };
            let position = native.position().map_err(|error| input_error(index, None, error))?;
            validate_moves(&position, native.candidates.iter().map(|candidate| candidate.mv))
                .map_err(|error| input_error(index, Some(&position.to_sfen(None)), error))?;
            native.candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
            let sfen =
                (import.report.issues.len() < ISSUE_EXAMPLES).then(|| position.to_sfen(None));
            import.loss(
                index,
                sfen.as_deref(),
                "ybb.ply",
                "native position identity excludes YBB move numbers",
            )?;
            import.push(
                index,
                || position.to_sfen(None),
                NativeBookPosition { entry: native, comment: None, move_comments: BTreeMap::new() },
            )?;
        }
        import.finish()
    }
}

impl SbkBook {
    /// SBK の局面を 1 件ずつ v4 へ変換し、状態評価や注釈を欠落として報告する。
    ///
    /// オプションの description が未指定の場合は SBK の description を保持する。
    /// 候補手検査は reader が返す解決済み行を対象とする。
    /// 未解決状態は欠落許可に従って集計し、重複診断は常に拒否する。
    ///
    /// # Errors
    /// 明示許可のない欠落、不正入力、I/O エラー、既存保存先がある場合に返す。
    /// 公開後の失敗は [`BookError::SaveUncertain`] を返し、出力が存在し得る。
    pub fn write_native_book(
        &self,
        path: impl AsRef<Path>,
        options: &NativeBookImportOptions,
    ) -> Result<NativeBookImportReport, BookError> {
        let mut metadata = options.metadata.clone();
        if metadata.description.is_none() {
            metadata.description = self.description().map(ToOwned::to_owned);
        }
        let mut import = Import::new(path.as_ref(), metadata, options)?;
        if self.diagnostics().duplicate_positions() > 0 {
            return Err(BookError::InvalidData(
                "native import rejects duplicate SBK positions".into(),
            ));
        }
        if self.author().is_some() {
            import.loss(0, None, "sbk.author", "native v4 has no author field")?;
        }
        for _ in 0..self.diagnostics().unresolved_next_states() {
            import.loss(
                0,
                None,
                "sbk.unresolved_state",
                "SBK reader could not resolve this source state position",
            )?;
            import.report.skipped_positions += 1;
        }
        for (index, entry) in self.iter_entries().enumerate() {
            let entry = entry?;
            if let Some(key) = import.position(
                index as u64,
                entry.sfen(),
                entry.moves().iter().map(|candidate| candidate.mv()),
            )? {
                let decoded = BookDatabase::entry_from_sbk(&entry)?;
                import.entry(index as u64, &decoded, key)?;
            }
        }
        import.finish()
    }
}

#[cfg(test)]
mod tests;
