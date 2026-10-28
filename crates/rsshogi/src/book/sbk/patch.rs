//! SBK の既存情報を保持する差分マージ出力。

use super::file::BufferedFile;
use std::io::{Seek, SeekFrom};

use super::*;
use crate::book::{
    BookDatabaseEdit, BookDatabaseEntry, BookDatabasePatch, BookPatchWritePhase,
    BookPatchWriteProgress,
};

impl SbkBook {
    /// 差分を反映した SBK を別の出力先へ逐次出力し、flush する。
    ///
    /// 元ファイルは処理中に変更してはいけない。既存候補手のリンクと未知フィールドを保持し、
    /// 新規候補手のリンクだけを導出する。出力先の永続化と置換は呼び出し側が行う。
    ///
    /// # Errors
    ///
    /// 保存不能なグラフ、表現不能な差分、全局面削除、読み書きの失敗時に返す。
    /// エラー時は部分出力が残り得るが、差分は変更しない。
    pub fn write_sbk_with_patch(
        &self,
        writer: &mut impl Write,
        patch: &BookDatabasePatch,
    ) -> Result<(), BookError> {
        self.write_sbk_with_patch_with_control(writer, patch, |_| BookControl::Continue)
    }

    /// 進捗通知と協調キャンセル付きで差分 SBK を逐次出力する。
    ///
    /// 件数は元 state の処理数であり、出力数や完了率ではない。入力と出力は別ファイルを使う。
    /// I/O 失敗やキャンセル後も同じ差分を新しい出力先へ再出力できる。
    ///
    /// # Errors
    ///
    /// [`Self::write_sbk_with_patch`] のエラーと [`BookError::Cancelled`] を返す。
    pub fn write_sbk_with_patch_with_control(
        &self,
        writer: &mut impl Write,
        patch: &BookDatabasePatch,
        mut control: impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        notify(&mut control, BookPatchWritePhase::Validating, 0, self.states.len())?;
        let mut input = BufferedFile::new(File::open(&self.path)?)?;
        let mut plan = Plan::new(self, patch, &mut control)?;
        let mut buffer = Vec::new();
        plan.validate(&mut input, &mut buffer, &mut control)?;
        plan.select_roots(&mut input, &mut buffer, &mut control)?;
        plan.write(&mut input, writer, &mut buffer, &mut control)
    }
}

struct Node<'a> {
    source: Option<usize>,
    edit: Option<&'a BookDatabaseEntry>,
    key: PackedSfen,
    ply: u32,
}

struct Plan<'a> {
    book: &'a SbkBook,
    nodes: Vec<Node<'a>>,
    old_to_new: Vec<Option<usize>>,
    by_key: Vec<([u8; 32], usize)>,
    roots: Vec<bool>,
}

fn invalid(message: &str) -> BookError {
    BookError::InvalidData(message.into())
}

fn notify(
    control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    phase: BookPatchWritePhase,
    count: usize,
    total: usize,
) -> Result<(), BookError> {
    BookPatchWriteProgress {
        phase,
        processed_positions: count as u64,
        total_positions: Some(total as u64),
    }
    .notify(control)
}

impl<'a> Plan<'a> {
    fn new(
        book: &'a SbkBook,
        patch: &'a BookDatabasePatch,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<Self, BookError> {
        if book.diagnostics.duplicate_positions != 0 || book.index.len() != book.states.len() {
            return Err(BookError::InvalidData(format!(
                "SBK patch requires unique resolved positions (duplicates: {}, unresolved: {})",
                book.diagnostics.duplicate_positions, book.diagnostics.unresolved_next_states
            )));
        }
        let mut nodes = Vec::new();
        let mut old_to_new = vec![None; book.states.len()];
        for (source, row) in book.state_lookup.iter().enumerate() {
            notify(control, BookPatchWritePhase::Validating, 0, book.states.len())?;
            let row = row.as_ref().ok_or_else(|| invalid("unresolved SBK state"))?;
            let mut position = Position::empty();
            position
                .set_packed_sfen(&row.key, false, row.ply)
                .map_err(|e| BookError::InvalidData(e.to_string()))?;
            let sfen = position.to_sfen(Some(1));
            let edit = match patch.entries.get(&sfen) {
                Some(BookDatabaseEdit::Delete) => continue,
                Some(BookDatabaseEdit::Upsert(entry)) => Some(entry.as_ref()),
                None => None,
            };
            old_to_new[source] = Some(nodes.len());
            let ply = edit
                .and_then(|entry| entry.position().original_ply())
                .unwrap_or(u32::from(row.ply));
            nodes.push(Node { source: Some(source), edit, key: row.key, ply });
        }
        for change in patch.entries.values() {
            notify(control, BookPatchWritePhase::Validating, 0, book.states.len())?;
            if let BookDatabaseEdit::Upsert(entry) = change {
                validate_patch_entry(entry)?;
                let key = entry
                    .position()
                    .packed_sfen()
                    .ok_or_else(|| invalid("SBK patch position is not packable"))?;
                if book.index.binary_search_by(|row| row.key.cmp_sbk_words(&key)).is_err() {
                    nodes.push(Node {
                        source: None,
                        edit: Some(entry),
                        key,
                        ply: entry.position().original_ply().unwrap_or(1),
                    });
                }
            }
        }
        if nodes.is_empty() {
            return Err(invalid("cannot write empty SBK database"));
        }
        usize_to_i32(nodes.len() - 1, "SBK state id")?;
        let mut by_key: Vec<_> =
            nodes.iter().enumerate().map(|(i, node)| (*node.key.as_bytes(), i)).collect();
        by_key.sort_unstable_by_key(|&(key, _)| key);
        let roots = vec![false; nodes.len()];
        Ok(Self { book, nodes, old_to_new, by_key, roots })
    }

    fn validate(
        &self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let total = self.book.states.len();
        notify(control, BookPatchWritePhase::Validating, 0, total)?;
        for source in 0..total {
            read_payload(self.book, input, source, buffer)?;
            validate_wire(buffer, Message::State)?;
            let raw = decode_state_payload(buffer)?;
            if raw.id.is_some_and(|id| id < 0) {
                return Err(invalid("negative SBK state id"));
            }
            let row = self.book.state_lookup[source]
                .as_ref()
                .ok_or_else(|| invalid("unresolved SBK state"))?;
            let mut position = Position::empty();
            // 辺の盤面一致だけを検査する。出力時の手数上限は check_output_plies で検査する。
            position
                .set_packed_sfen(&row.key, false, 1)
                .map_err(|e| BookError::InvalidData(e.to_string()))?;
            let mut seen = HashSet::new();
            for candidate in &raw.moves {
                let word = candidate.raw_move.ok_or_else(|| invalid("SBK move word is missing"))?;
                let mv = sbk_move_word_to_move(word)?;
                if !seen.insert(mv) {
                    return Err(invalid("duplicate SBK candidate"));
                }
                let mv32 = checked_move32(&position, mv)?;
                if let Some(id) = candidate.next_state_id.filter(|&id| id != -1) {
                    let child = self
                        .book
                        .state_id_to_index
                        .get(&id)
                        .ok_or_else(|| invalid("dangling SBK next state id"))?;
                    let target = self.book.state_lookup[*child]
                        .as_ref()
                        .ok_or_else(|| invalid("unresolved SBK next state"))?;
                    position.try_apply_move32(mv32).map_err(|e| {
                        invalid(&format!("failed to apply SBK validation move: {e:?}"))
                    })?;
                    let key = packed_sfen_of_state(&position)?;
                    position
                        .undo_move32(mv32)
                        .map_err(|e| BookError::InvalidData(format!("SBK undo failed: {e:?}")))?;
                    if key != target.key {
                        return Err(invalid("inconsistent SBK graph edge"));
                    }
                }
            }
            if let Some(output) = self.old_to_new[source]
                && let Some(edit) = self.nodes[output].edit
            {
                check_eval_replacement(buffer, &raw, edit)?;
            }
            notify(control, BookPatchWritePhase::Validating, source + 1, total)?;
        }
        // 差分の合法性も出力開始前に検査する。
        for (index, node) in self.nodes.iter().enumerate() {
            if node.edit.is_some() {
                self.links(input, buffer, index)?;
            }
            notify(control, BookPatchWritePhase::Validating, total, total)?;
        }
        Ok(())
    }

    fn raw(
        &self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        index: usize,
    ) -> Result<RawSbkState, BookError> {
        if let Some(source) = self.nodes[index].source {
            read_payload(self.book, input, source, buffer)?;
            decode_state_payload(buffer)
        } else {
            buffer.clear();
            Ok(RawSbkState::default())
        }
    }

    fn translated_link(&self, old: Option<i32>) -> Result<Option<i32>, BookError> {
        match old {
            None | Some(-1) => Ok(old),
            Some(id) => {
                let source = *self
                    .book
                    .state_id_to_index
                    .get(&id)
                    .ok_or_else(|| invalid("dangling SBK next state id"))?;
                self.old_to_new[source]
                    .map_or(Ok(Some(-1)), |out| usize_to_i32(out, "SBK state id").map(Some))
            }
        }
    }

    fn links(
        &self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        index: usize,
    ) -> Result<Vec<Option<i32>>, BookError> {
        let raw = self.raw(input, buffer, index)?;
        self.links_from_raw(index, &raw)
    }

    fn links_from_raw(
        &self,
        index: usize,
        raw: &RawSbkState,
    ) -> Result<Vec<Option<i32>>, BookError> {
        let node = &self.nodes[index];
        let Some(edit) = node.edit else {
            return raw.moves.iter().map(|mv| self.translated_link(mv.next_state_id)).collect();
        };
        let mut position = Position::from_sfen(edit.position().sfen())
            .map_err(|e| BookError::InvalidData(e.to_string()))?;
        edit.candidates()
            .iter()
            .map(|candidate| {
                let mv32 = checked_move32(&position, candidate.mv())?;
                if let Some(old) = raw.moves.iter().find(|old| {
                    old.raw_move.and_then(|word| sbk_move_word_to_move(word).ok())
                        == Some(candidate.mv())
                }) {
                    return self.translated_link(old.next_state_id);
                }
                position
                    .try_apply_move32(mv32)
                    .map_err(|e| invalid(&format!("failed to apply SBK patch move: {e:?}")))?;
                let child = packed_sfen_of_state(&position)?;
                position
                    .undo_move32(mv32)
                    .map_err(|e| BookError::InvalidData(format!("SBK undo failed: {e:?}")))?;
                Ok(Some(
                    match self.by_key.binary_search_by_key(child.as_bytes(), |&(key, _)| key) {
                        Ok(found) => usize_to_i32(self.by_key[found].1, "SBK state id")?,
                        Err(_) => -1,
                    },
                ))
            })
            .collect()
    }

    fn select_roots(
        &mut self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let total = self.book.states.len();
        let mut incoming = vec![false; self.nodes.len()];
        for index in 0..self.nodes.len() {
            let raw = self.raw(input, buffer, index)?;
            self.roots[index] = raw.position.is_some()
                || index == 0
                || self.nodes[index]
                    .edit
                    .is_some_and(|entry| entry.position().original_ply().is_some());
            for link in
                self.links_from_raw(index, &raw)?.into_iter().flatten().filter(|&id| id >= 0)
            {
                incoming[link as usize] = true;
            }
            notify(control, BookPatchWritePhase::Validating, total, total)?;
        }
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = VecDeque::new();
        for index in 0..self.nodes.len() {
            if self.roots[index] {
                self.visit(input, buffer, index, &mut seen, &mut queue, control)?;
            }
        }
        for (index, &has_incoming) in incoming.iter().enumerate() {
            if !has_incoming && !seen[index] {
                self.roots[index] = true;
                self.visit(input, buffer, index, &mut seen, &mut queue, control)?;
            }
        }
        for index in 0..self.nodes.len() {
            if !seen[index] {
                self.roots[index] = true;
                self.visit(input, buffer, index, &mut seen, &mut queue, control)?;
            }
        }
        self.check_output_plies(input, buffer, control)
    }

    fn visit(
        &self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        root: usize,
        seen: &mut [bool],
        queue: &mut VecDeque<usize>,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        if seen[root] {
            return Ok(());
        }
        seen[root] = true;
        queue.push_back(root);
        while let Some(index) = queue.pop_front() {
            for link in
                self.links(input, buffer, index)?.into_iter().flatten().filter(|&id| id >= 0)
            {
                let child = link as usize;
                if !seen[child] {
                    seen[child] = true;
                    queue.push_back(child);
                }
            }
            notify(
                control,
                BookPatchWritePhase::Validating,
                self.book.states.len(),
                self.book.states.len(),
            )?;
        }
        Ok(())
    }

    fn check_output_plies(
        &self,
        input: &mut BufferedFile,
        buffer: &mut Vec<u8>,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        // 補完起点が確定してから、reader と同じ物理順で走査する。
        let mut seen = vec![false; self.nodes.len()];
        let mut queue = VecDeque::new();
        for root in 0..self.nodes.len() {
            if !self.roots[root] || seen[root] {
                continue;
            }
            seen[root] = true;
            queue.push_back((root, self.nodes[root].ply));
            while let Some((index, inherited_ply)) = queue.pop_front() {
                let ply = if self.roots[index] { self.nodes[index].ply } else { inherited_ply };
                for link in
                    self.links(input, buffer, index)?.into_iter().flatten().filter(|&id| id >= 0)
                {
                    let child = link as usize;
                    if !seen[child] {
                        let next_ply = ply
                            .checked_add(1)
                            .filter(|&ply| ply <= u32::from(Ply::MAX))
                            .ok_or_else(|| {
                                invalid("SBK traversal ply exceeds the supported SFEN range")
                            })?;
                        seen[child] = true;
                        queue.push_back((child, next_ply));
                    }
                }
                notify(
                    control,
                    BookPatchWritePhase::Validating,
                    self.book.states.len(),
                    self.book.states.len(),
                )?;
            }
        }
        Ok(())
    }

    fn write(
        &self,
        input: &mut BufferedFile,
        writer: &mut impl Write,
        buffer: &mut Vec<u8>,
        control: &mut impl FnMut(BookPatchWriteProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let total = self.book.states.len();
        notify(control, BookPatchWritePhase::Writing, 0, total)?;
        input.rewind()?;
        let size = input.len();
        let mut source = 0;
        while let Some(field) = super::file::next_field(input, size)? {
            if field.field == FIELD_SBOOK_STATE && field.wire_type == 2 {
                if let Some(index) = self.old_to_new[source] {
                    self.write_node(input, writer, buffer, index)?;
                }
                source += 1;
                notify(control, BookPatchWritePhase::Writing, source, total)?;
            } else {
                super::file::copy_span(input, writer, field.start, field.end)?;
            }
            input.seek(SeekFrom::Start(field.end))?;
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.source.is_none() {
                self.write_node(input, writer, buffer, index)?;
                notify(control, BookPatchWritePhase::Writing, total, total)?;
            }
        }
        notify(control, BookPatchWritePhase::Writing, total, total)?;
        writer.flush()?;
        Ok(())
    }

    fn write_node(
        &self,
        input: &mut BufferedFile,
        writer: &mut impl Write,
        buffer: &mut Vec<u8>,
        index: usize,
    ) -> Result<(), BookError> {
        let raw = self.raw(input, buffer, index)?;
        let links = self.links_from_raw(index, &raw)?;
        let node = &self.nodes[index];
        let mut output = Vec::new();
        write_field_i32(&mut output, FIELD_STATE_ID, usize_to_i32(index, "SBK state id")?);
        if let Some(edit) = node.edit {
            encode_edit(&mut output, buffer, &raw, edit, &links)?;
        } else {
            for field in fields(buffer)? {
                match field.number {
                    FIELD_STATE_ID | FIELD_STATE_POSITION | FIELD_STATE_MOVES => {}
                    _ => output.extend_from_slice(field.bytes),
                }
            }
            for (i, field) in fields(buffer)?
                .into_iter()
                .filter(|field| field.number == FIELD_STATE_MOVES)
                .enumerate()
            {
                let payload = replace_link(field.payload, links[i])?;
                write_field_message(&mut output, FIELD_STATE_MOVES, &payload)?;
            }
        }
        if self.roots[index] {
            let sfen = if node.edit.is_none() && raw.position.is_some() {
                raw.position.clone().unwrap_or_default()
            } else {
                let mut position = Position::empty();
                position
                    .set_packed_sfen(&node.key, false, 1)
                    .map_err(|e| BookError::InvalidData(e.to_string()))?;
                position.to_sfen(Some(
                    i32::try_from(node.ply).map_err(|_| invalid("SBK ply is too large"))?,
                ))
            };
            write_field_string(&mut output, FIELD_STATE_POSITION, &sfen)?;
        }
        let mut header = Vec::new();
        write_field_tag(&mut header, FIELD_SBOOK_STATE, 2);
        write_varint(&mut header, output.len() as u64);
        writer.write_all(&header)?;
        writer.write_all(&output)?;
        Ok(())
    }
}

fn read_payload(
    book: &SbkBook,
    input: &mut BufferedFile,
    source: usize,
    buffer: &mut Vec<u8>,
) -> Result<(), BookError> {
    let state = &book.states[source];
    input.seek(SeekFrom::Start(state.payload_offset))?;
    buffer.resize(state.payload_bytes, 0);
    input.read_exact(buffer)?;
    Ok(())
}

struct Field<'a> {
    number: u32,
    wire: u32,
    bytes: &'a [u8],
    payload: &'a [u8],
}

fn fields(bytes: &[u8]) -> Result<Vec<Field<'_>>, BookError> {
    let mut input = WireInput::new(bytes);
    let mut result = Vec::new();
    while !input.is_empty() {
        let start = input.pos;
        let (number, wire) = input.read_tag()?;
        let payload = if wire == 2 {
            input.read_bytes()?
        } else {
            input.skip_value(wire)?;
            &[]
        };
        result.push(Field { number, wire, bytes: &bytes[start..input.pos], payload });
    }
    Ok(result)
}

#[derive(Clone, Copy)]
enum Message {
    State,
    Move,
    Eval,
}

fn validate_wire(bytes: &[u8], message: Message) -> Result<(), BookError> {
    let mut seen = HashSet::new();
    for field in fields(bytes)? {
        let expected = match message {
            Message::State => match field.number {
                1..=6 => Some(0),
                7..=10 => Some(2),
                _ => None,
            },
            Message::Move => (1..=4).contains(&field.number).then_some(0),
            Message::Eval => match field.number {
                1..=4 => Some(0),
                5..=6 => Some(2),
                _ => None,
            },
        };
        if let Some(wire) = expected {
            if field.wire != wire {
                return Err(invalid("invalid SBK known field wire type"));
            }
            let repeated = matches!(message, Message::State) && matches!(field.number, 9 | 10);
            if !repeated && !seen.insert(field.number) {
                return Err(invalid("duplicate SBK singular field"));
            }
        }
        if matches!(message, Message::State) {
            match field.number {
                FIELD_STATE_MOVES => validate_wire(field.payload, Message::Move)?,
                FIELD_STATE_EVALS => validate_wire(field.payload, Message::Eval)?,
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_patch_entry(entry: &BookDatabaseEntry) -> Result<(), BookError> {
    if entry.position().original_ply().is_some_and(|ply| ply > u32::from(Ply::MAX)) {
        return Err(invalid("SBK patch ply exceeds the supported SFEN range"));
    }
    if entry.metadata().yaneuraou().is_some() {
        return Err(BookError::Unsupported("SBK patch cannot retain DB2016 metadata"));
    }
    for mv in entry.candidates() {
        if mv.ponder().is_some()
            || mv.depth().is_some()
            || mv.count().is_some()
            || mv.metadata().yaneuraou().is_some()
        {
            return Err(BookError::Unsupported("SBK patch contains unrepresentable fields"));
        }
        if mv.score().is_some()
            && mv.score() != mv.metadata().sbk().and_then(|meta| meta.evaluation())
        {
            return Err(BookError::Unsupported("SBK patch score is not move evaluation metadata"));
        }
    }
    Ok(())
}

fn evals_unchanged(raw: &RawSbkState, edit: &BookDatabaseEntry) -> bool {
    let source: Vec<_> = raw
        .evals
        .iter()
        .map(|e| {
            SbkEvalMetadata::new(
                e.evaluation_value,
                e.depth,
                e.sel_depth,
                e.nodes,
                e.variation.clone(),
                e.engine_name.clone(),
            )
        })
        .collect();
    edit.metadata().sbk().map_or(source.is_empty(), |m| m.evals() == source)
}

fn check_eval_replacement(
    bytes: &[u8],
    raw: &RawSbkState,
    edit: &BookDatabaseEntry,
) -> Result<(), BookError> {
    if !evals_unchanged(raw, edit) {
        for eval in fields(bytes)?.into_iter().filter(|f| f.number == FIELD_STATE_EVALS) {
            if fields(eval.payload)?.iter().any(|f| !(1..=6).contains(&f.number)) {
                return Err(BookError::Unsupported(
                    "SBK patch cannot replace evals with unknown fields",
                ));
            }
        }
    }
    Ok(())
}

fn replace_link(bytes: &[u8], link: Option<i32>) -> Result<Vec<u8>, BookError> {
    let mut output = Vec::new();
    for field in fields(bytes)? {
        if field.number != FIELD_MOVE_NEXT_STATE_ID {
            output.extend_from_slice(field.bytes);
        }
    }
    if let Some(link) = link {
        write_field_i32(&mut output, FIELD_MOVE_NEXT_STATE_ID, link);
    }
    Ok(output)
}

fn encode_edit(
    output: &mut Vec<u8>,
    bytes: &[u8],
    raw: &RawSbkState,
    edit: &BookDatabaseEntry,
    links: &[Option<i32>],
) -> Result<(), BookError> {
    for field in fields(bytes)? {
        if !(1..=10).contains(&field.number) {
            output.extend_from_slice(field.bytes);
        }
    }
    write_field_u64(output, FIELD_STATE_BOARD_KEY, 0);
    write_field_u64(output, FIELD_STATE_HAND_KEY, 0);
    if let Some(metadata) = edit.metadata().sbk() {
        for (field, value) in [
            (FIELD_STATE_GAMES, metadata.games()),
            (FIELD_STATE_WON_BLACK, metadata.won_black()),
            (FIELD_STATE_WON_WHITE, metadata.won_white()),
        ] {
            if let Some(value) = value {
                write_field_i32(output, field, value);
            }
        }
        if let Some(comment) = metadata.comment() {
            write_field_string(output, FIELD_STATE_COMMENT, comment)?;
        }
    }
    let original_moves: Vec<_> =
        fields(bytes)?.into_iter().filter(|f| f.number == FIELD_STATE_MOVES).collect();
    let position = Position::from_sfen(edit.position().sfen())
        .map_err(|e| BookError::InvalidData(e.to_string()))?;
    for (index, candidate) in edit.candidates().iter().enumerate() {
        let mut payload = Vec::new();
        if let Some(old) = raw.moves.iter().position(|old| {
            old.raw_move.and_then(|word| sbk_move_word_to_move(word).ok()) == Some(candidate.mv())
        }) {
            for field in fields(original_moves[old].payload)? {
                if !(1..=4).contains(&field.number) {
                    payload.extend_from_slice(field.bytes);
                }
            }
        }
        write_field_i32(
            &mut payload,
            FIELD_MOVE_MOVE,
            sbk_move_word_from_position_move(&position, candidate.mv())?,
        );
        let evaluation = candidate.metadata().sbk().map_or(Some(0), |meta| meta.evaluation());
        if let Some(value) = evaluation {
            write_field_i32(&mut payload, FIELD_MOVE_EVALUATION, value);
        }
        if let Some(value) = sbk_candidate_weight(candidate) {
            write_field_i32(&mut payload, FIELD_MOVE_WEIGHT, value);
        }
        if let Some(link) = links[index] {
            write_field_i32(&mut payload, FIELD_MOVE_NEXT_STATE_ID, link);
        }
        write_field_message(output, FIELD_STATE_MOVES, &payload)?;
    }
    if evals_unchanged(raw, edit) {
        for field in fields(bytes)?.into_iter().filter(|f| f.number == FIELD_STATE_EVALS) {
            output.extend_from_slice(field.bytes);
        }
    } else if let Some(metadata) = edit.metadata().sbk() {
        for eval in metadata.evals() {
            write_field_message(output, FIELD_STATE_EVALS, &encode_sbk_eval(eval)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
