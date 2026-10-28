//! 局面索引に必要な情報を取り出し、残る既知フィールドの形式を検査する。

use super::*;

pub(super) fn decode<'a>(
    payload: &'a [u8],
    moves: &mut Vec<RawSbkMove>,
) -> Result<Option<&'a str>, BookError> {
    let mut input = WireInput::new(payload);
    let mut position = None;
    moves.clear();
    while !input.is_empty() {
        let (field, wire) = input.read_tag()?;
        match (field, wire) {
            (
                FIELD_STATE_ID | FIELD_STATE_GAMES | FIELD_STATE_WON_BLACK | FIELD_STATE_WON_WHITE,
                0,
            ) => {
                input.read_i32()?;
            }
            (FIELD_STATE_POSITION, 2) => position = Some(read_str(&mut input)?),
            (FIELD_STATE_COMMENT, 2) => {
                read_str(&mut input)?;
            }
            (FIELD_STATE_MOVES, 2) => moves.push(decode_move_payload(input.read_bytes()?)?),
            (FIELD_STATE_EVALS, 2) => validate_eval(input.read_bytes()?)?,
            _ => input.skip_value(wire)?,
        }
    }
    Ok(position)
}

fn read_str<'a>(input: &mut WireInput<'a>) -> Result<&'a str, BookError> {
    std::str::from_utf8(input.read_bytes()?)
        .map_err(|_| BookError::InvalidFormat("protobuf string is not UTF-8"))
}

fn validate_eval(payload: &[u8]) -> Result<(), BookError> {
    let mut input = WireInput::new(payload);
    while !input.is_empty() {
        let (field, wire) = input.read_tag()?;
        match (field, wire) {
            (FIELD_EVAL_VALUE | FIELD_EVAL_DEPTH | FIELD_EVAL_SEL_DEPTH | FIELD_EVAL_NODES, 0) => {
                input.read_varint()?;
            }
            (FIELD_EVAL_VARIATION | FIELD_EVAL_ENGINE_NAME, 2) => {
                read_str(&mut input)?;
            }
            _ => input.skip_value(wire)?,
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
