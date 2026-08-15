use std::collections::HashMap;
use std::io::Write;

use crate::book::{BookDatabase, BookDatabaseEntry, BookError};

const HEADER: &str = "#YANEURAOU-DB2016 1.00";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct YaneuraOuDb2016WriteOptions {
    preserve_move_order: bool,
    fixed_ply: Option<u32>,
    omit_ply: bool,
    keep_last_on_duplicate: bool,
    emit_noe: bool,
}

impl YaneuraOuDb2016WriteOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            preserve_move_order: false,
            fixed_ply: None,
            omit_ply: false,
            keep_last_on_duplicate: false,
            emit_noe: false,
        }
    }

    #[must_use]
    pub const fn with_preserved_move_order(mut self) -> Self {
        self.preserve_move_order = true;
        self
    }

    #[must_use]
    pub const fn with_fixed_ply(mut self, ply: u32) -> Self {
        self.fixed_ply = Some(ply);
        self
    }

    #[must_use]
    pub const fn with_omitted_ply(mut self) -> Self {
        self.omit_ply = true;
        self
    }

    #[must_use]
    pub const fn with_keep_last_on_duplicate(mut self) -> Self {
        self.keep_last_on_duplicate = true;
        self
    }

    #[must_use]
    pub const fn with_noe(mut self, emit: bool) -> Self {
        self.emit_noe = emit;
        self
    }
}

impl Default for YaneuraOuDb2016WriteOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl BookDatabase {
    pub fn write_yaneuraou_db2016(
        &self,
        w: &mut impl Write,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<(), BookError> {
        w.write_all(self.to_yaneuraou_db2016_string(options)?.as_bytes())?;
        Ok(())
    }

    pub fn to_yaneuraou_db2016_string(
        &self,
        options: &YaneuraOuDb2016WriteOptions,
    ) -> Result<String, BookError> {
        let mut by_sfen: HashMap<&str, &BookDatabaseEntry> = HashMap::new();
        for entry in self.entries() {
            let key = entry.position().sfen();
            if by_sfen.contains_key(key) && !options.keep_last_on_duplicate {
                return Err(BookError::InvalidData(format!("duplicate DB2016 position: {key}")));
            }
            by_sfen.insert(key, entry);
        }

        let mut entries: Vec<_> = by_sfen.into_values().collect();
        entries.sort_by(|left, right| {
            left.position().sfen().as_bytes().cmp(right.position().sfen().as_bytes())
        });

        let mut output = String::new();
        output.push_str(HEADER);
        output.push('\n');
        if options.emit_noe {
            output.push_str(&format!("# NOE:{}\n", entries.len()));
        }

        for entry in entries {
            write_entry(&mut output, entry, options)?;
        }
        Ok(output)
    }
}

fn write_entry(
    output: &mut String,
    entry: &BookDatabaseEntry,
    options: &YaneuraOuDb2016WriteOptions,
) -> Result<(), BookError> {
    let position = entry.position();
    let mut fields = position.sfen().split_ascii_whitespace();
    let board = fields.next().ok_or(BookError::InvalidData("missing normalized board".into()))?;
    let side = fields.next().ok_or(BookError::InvalidData("missing normalized side".into()))?;
    let hands = fields.next().ok_or(BookError::InvalidData("missing normalized hands".into()))?;
    output.push_str("sfen ");
    output.push_str(board);
    output.push(' ');
    output.push_str(side);
    output.push(' ');
    output.push_str(hands);
    if !options.omit_ply {
        let ply = options
            .fixed_ply
            .or(position.original_ply())
            .or_else(|| entry.metadata().yaneuraou().map(|metadata| metadata.min_ply()))
            .unwrap_or(1);
        if ply != 0 {
            output.push(' ');
            output.push_str(&ply.to_string());
        }
    }
    output.push('\n');

    if let Some(metadata) = entry.metadata().yaneuraou() {
        write_comments(output, metadata.comment());
    }

    let mut candidates: Vec<_> = entry.candidates().iter().collect();
    if !options.preserve_move_order {
        candidates.sort_by(|left, right| match (left.score(), right.score()) {
            (Some(left), Some(right)) => right.cmp(&left),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
    }

    for candidate in candidates {
        if !is_valid_db2016_move(candidate.mv()) {
            return Err(BookError::InvalidData(format!(
                "invalid DB2016 move: {:#06x}",
                candidate.mv().raw()
            )));
        }
        if candidate.ponder().is_some_and(|ponder| !is_valid_db2016_move(ponder)) {
            return Err(BookError::InvalidData("invalid DB2016 ponder move".into()));
        }
        output.push_str(&candidate.mv().to_usi());
        output.push(' ');
        output.push_str(&candidate.ponder().map_or_else(|| "none".to_string(), |mv| mv.to_usi()));
        output.push(' ');
        write_optional(output, candidate.score());
        output.push(' ');
        write_optional(output, candidate.depth());
        if let Some(count) = candidate.count() {
            output.push(' ');
            output.push_str(&count.to_string());
        }
        output.push('\n');
        if let Some(metadata) = candidate.metadata().yaneuraou() {
            write_comments(output, metadata.comment());
        }
    }
    Ok(())
}

fn write_optional<T: std::fmt::Display>(output: &mut String, value: Option<T>) {
    match value {
        Some(value) => output.push_str(&value.to_string()),
        None => output.push_str("none"),
    }
}

fn write_comments(output: &mut String, comment: &str) {
    if comment.is_empty() {
        return;
    }
    for line in comment.split('\n') {
        output.push_str("# ");
        output.push_str(line);
        output.push('\n');
    }
}

fn is_valid_db2016_move(mv: crate::types::Move) -> bool {
    if !mv.is_normal() || mv.to_sq().raw() < 0 || mv.to_sq().raw() >= 81 {
        return false;
    }
    if mv.is_drop() {
        !mv.is_promotion() && matches!((mv.raw() >> 7) & 0x7f, 1..=7)
    } else {
        mv.from_sq().raw() >= 0 && mv.from_sq().raw() < 81 && mv.from_sq() != mv.to_sq()
    }
}
