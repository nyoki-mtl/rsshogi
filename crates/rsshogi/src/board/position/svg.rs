use super::Position;
use crate::types::{Color, Hand, HandPiece, Move32, Piece, PieceType, Square};
use std::fmt::Write;

const SVG_PIECE_DEFS: [&str; 28] = [
    "<g id=\"black-pawn\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">歩</text></g>",
    "<g id=\"black-lance\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">香</text></g>",
    "<g id=\"black-knight\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">桂</text></g>",
    "<g id=\"black-silver\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">銀</text></g>",
    "<g id=\"black-gold\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">金</text></g>",
    "<g id=\"black-bishop\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">角</text></g>",
    "<g id=\"black-rook\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">飛</text></g>",
    "<g id=\"black-king\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">王</text></g>",
    "<g id=\"black-pro-pawn\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">と</text></g>",
    "<g id=\"black-pro-lance\" transform=\"scale(1.0, 0.5)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"18\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"34\">香</text></g>",
    "<g id=\"black-pro-knight\" transform=\"scale(1.0, 0.5)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"18\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"34\">桂</text></g>",
    "<g id=\"black-pro-silver\" transform=\"scale(1.0, 0.5)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"18\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"34\">銀</text></g>",
    "<g id=\"black-horse\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">馬</text></g>",
    "<g id=\"black-dragon\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"10.5\" y=\"16.5\">龍</text></g>",
    "<g id=\"white-pawn\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">歩</text></g>",
    "<g id=\"white-lance\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">香</text></g>",
    "<g id=\"white-knight\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">桂</text></g>",
    "<g id=\"white-silver\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">銀</text></g>",
    "<g id=\"white-gold\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">金</text></g>",
    "<g id=\"white-bishop\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">角</text></g>",
    "<g id=\"white-rook\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">飛</text></g>",
    "<g id=\"white-king\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">王</text></g>",
    "<g id=\"white-pro-pawn\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">と</text></g>",
    "<g id=\"white-pro-lance\" transform=\"scale(1.0, 0.5) rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-22\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-6\">香</text></g>",
    "<g id=\"white-pro-knight\" transform=\"scale(1.0, 0.5) rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-22\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-6\">桂</text></g>",
    "<g id=\"white-pro-silver\" transform=\"scale(1.0, 0.5) rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-22\">成</text><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-6\">銀</text></g>",
    "<g id=\"white-horse\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">馬</text></g>",
    "<g id=\"white-dragon\" transform=\"rotate(180)\"><text font-family=\"serif\" font-size=\"17\" text-anchor=\"middle\" x=\"-10.5\" y=\"-3.5\">龍</text></g>",
];

const SVG_SQUARES: &str = "<g stroke=\"black\"><rect x=\"20\" y=\"10\" width=\"181\" height=\"181\" fill=\"none\" stroke-width=\"1.5\" /><line x1=\"20.5\" y1=\"30.5\" x2=\"200.5\" y2=\"30.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"50.5\" x2=\"200.5\" y2=\"50.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"70.5\" x2=\"200.5\" y2=\"70.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"90.5\" x2=\"200.5\" y2=\"90.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"110.5\" x2=\"200.5\" y2=\"110.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"130.5\" x2=\"200.5\" y2=\"130.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"150.5\" x2=\"200.5\" y2=\"150.5\" stroke-width=\"1.0\" /><line x1=\"20.5\" y1=\"170.5\" x2=\"200.5\" y2=\"170.5\" stroke-width=\"1.0\" /><line x1=\"40.5\" y1=\"10.5\" x2=\"40.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"60.5\" y1=\"10.5\" x2=\"60.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"80.5\" y1=\"10.5\" x2=\"80.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"100.5\" y1=\"10.5\" x2=\"100.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"120.5\" y1=\"10.5\" x2=\"120.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"140.5\" y1=\"10.5\" x2=\"140.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"160.5\" y1=\"10.5\" x2=\"160.5\" y2=\"190.5\" stroke-width=\"1.0\" /><line x1=\"180.5\" y1=\"10.5\" x2=\"180.5\" y2=\"190.5\" stroke-width=\"1.0\" /></g>";

const SVG_COORDINATES: &str = "<g><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"30.5\" y=\"8\">9</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"50.5\" y=\"8\">8</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"70.5\" y=\"8\">7</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"90.5\" y=\"8\">6</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"110.5\" y=\"8\">5</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"130.5\" y=\"8\">4</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"150.5\" y=\"8\">3</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"170.5\" y=\"8\">2</text><text font-family=\"serif\" text-anchor=\"middle\" font-size=\"9\" x=\"190.5\" y=\"8\">1</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"23\">一</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"43\">二</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"63\">三</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"83\">四</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"103\">五</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"123\">六</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"143\">七</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"163\">八</text><text font-family=\"serif\" font-size=\"9\" x=\"203.5\" y=\"183\">九</text></g>";

const NUMBER_JAPANESE_KANJI_SYMBOLS: [&str; 19] = [
    "", "一", "二", "三", "四", "五", "六", "七", "八", "九", "十", "十一", "十二", "十三", "十四",
    "十五", "十六", "十七", "十八",
];

const HAND_PIECE_ORDER: [PieceType; 7] = [
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::GOLD,
    PieceType::BISHOP,
    PieceType::ROOK,
];

const HAND_PIECE_SYMBOLS: [&str; 7] = ["歩", "香", "桂", "銀", "金", "角", "飛"];

impl Position {
    /// 局面を SVG 文字列として描画する。
    ///
    /// `last_move` を指定すると、移動元と移動先をハイライトする。
    /// `scale` が 0 以下の場合は `1.0` として扱う。
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::board;
    /// use rsshogi::types::Move;
    ///
    /// board::init();
    ///
    /// let mut pos = board::hirate_position();
    /// let mv = pos.move32_from_move(Move::from_usi("7g7f").unwrap());
    /// pos.apply_move32(mv);
    ///
    /// let svg = pos.to_svg(Some(mv), 1.0);
    /// assert!(svg.starts_with("<svg"));
    /// ```
    #[must_use]
    pub fn to_svg(&self, last_move: Option<Move32>, scale: f32) -> String {
        let scale = if scale <= 0.0 { 1.0 } else { scale };
        let width = 230.0;
        let height = 192.0;

        let mut out = String::new();
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
            width * scale,
            height * scale,
            width,
            height
        );
        out.push_str("<defs>");
        for def in SVG_PIECE_DEFS {
            out.push_str(def);
        }
        out.push_str("</defs>");

        if let Some(mv) = last_move.filter(|mv| mv.is_normal()) {
            let to = mv.to_sq();
            if let Some((x, y)) = svg_square_coords(to) {
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"20\" height=\"20\" fill=\"#f6b94d\" />",
                    x, y
                );
            }
            if !mv.is_drop() {
                let from = mv.from_sq();
                if let Some((x, y)) = svg_square_coords(from) {
                    let _ = write!(
                        out,
                        "<rect x=\"{}\" y=\"{}\" width=\"20\" height=\"20\" fill=\"#fdf0e3\" />",
                        x, y
                    );
                }
            }
        }

        out.push_str(SVG_SQUARES);
        out.push_str(SVG_COORDINATES);

        for idx in 0..81 {
            let sq = Square::from_index(idx);
            let piece = self.piece_on(sq);
            if piece == Piece::NONE {
                continue;
            }
            let Some(id) = svg_piece_id(piece) else {
                continue;
            };
            let Some((x, y)) = svg_square_coords(sq) else {
                continue;
            };
            let _ = write!(out, "<use xlink:href=\"#{id}\" x=\"{x}\" y=\"{y}\" />");
        }

        for color in [Color::BLACK, Color::WHITE] {
            let texts = svg_hand_texts(self.hand(color), color);
            let mut hand_scale = 1.0;
            if texts.len() + 1 > 13 {
                hand_scale = 13.0 / (texts.len() as f32 + 1.0);
            }
            let (x, y) = if color == Color::BLACK { (214.0, 190.0) } else { (-16.0, -10.0) };
            for (idx, text) in texts.into_iter().enumerate() {
                let font_size = 14.0 * hand_scale;
                let y = y - font_size * idx as f32;
                let _ = write!(
                    out,
                    "<text font-family=\"serif\" font-size=\"{}\" x=\"{}\" y=\"{}\"{}>{}</text>",
                    font_size,
                    x,
                    y,
                    if color == Color::WHITE { " transform=\"rotate(180)\"" } else { "" },
                    text
                );
            }
        }

        out.push_str("</svg>");
        out
    }
}

fn svg_square_coords(sq: Square) -> Option<(f32, f32)> {
    if !sq.is_valid() || sq.is_none() {
        return None;
    }
    let file = sq.file().raw() as f32;
    let rank = sq.rank().raw() as f32;
    let x = 20.5 + (8.0 - file) * 20.0;
    let y = 10.5 + rank * 20.0;
    Some((x, y))
}

fn svg_piece_id(piece: Piece) -> Option<&'static str> {
    if piece == Piece::NONE {
        return None;
    }
    let prefix = match piece.color() {
        Color::BLACK => "black",
        Color::WHITE => "white",
    };
    let suffix = match piece.piece_type() {
        PieceType::PAWN => "pawn",
        PieceType::LANCE => "lance",
        PieceType::KNIGHT => "knight",
        PieceType::SILVER => "silver",
        PieceType::GOLD => "gold",
        PieceType::BISHOP => "bishop",
        PieceType::ROOK => "rook",
        PieceType::KING => "king",
        PieceType::PRO_PAWN => "pro-pawn",
        PieceType::PRO_LANCE => "pro-lance",
        PieceType::PRO_KNIGHT => "pro-knight",
        PieceType::PRO_SILVER => "pro-silver",
        PieceType::HORSE => "horse",
        PieceType::DRAGON => "dragon",
        _ => return None,
    };
    Some(match prefix {
        "black" => match suffix {
            "pawn" => "black-pawn",
            "lance" => "black-lance",
            "knight" => "black-knight",
            "silver" => "black-silver",
            "gold" => "black-gold",
            "bishop" => "black-bishop",
            "rook" => "black-rook",
            "king" => "black-king",
            "pro-pawn" => "black-pro-pawn",
            "pro-lance" => "black-pro-lance",
            "pro-knight" => "black-pro-knight",
            "pro-silver" => "black-pro-silver",
            "horse" => "black-horse",
            "dragon" => "black-dragon",
            _ => return None,
        },
        "white" => match suffix {
            "pawn" => "white-pawn",
            "lance" => "white-lance",
            "knight" => "white-knight",
            "silver" => "white-silver",
            "gold" => "white-gold",
            "bishop" => "white-bishop",
            "rook" => "white-rook",
            "king" => "white-king",
            "pro-pawn" => "white-pro-pawn",
            "pro-lance" => "white-pro-lance",
            "pro-knight" => "white-pro-knight",
            "pro-silver" => "white-pro-silver",
            "horse" => "white-horse",
            "dragon" => "white-dragon",
            _ => return None,
        },
        _ => return None,
    })
}

fn svg_hand_texts(hand: Hand, color: Color) -> Vec<String> {
    let mut items = Vec::new();
    for (piece_type, symbol) in HAND_PIECE_ORDER.iter().zip(HAND_PIECE_SYMBOLS.iter()) {
        let count = HandPiece::from_piece_type(*piece_type).map(|hp| hand.count(hp)).unwrap_or(0);
        if count >= 11 {
            let ones = (count % 10) as usize;
            if ones > 0 {
                items.push(NUMBER_JAPANESE_KANJI_SYMBOLS[ones].to_string());
            }
            items.push(NUMBER_JAPANESE_KANJI_SYMBOLS[10].to_string());
        } else if count >= 2 {
            items.push(NUMBER_JAPANESE_KANJI_SYMBOLS[count as usize].to_string());
        }
        if count >= 1 {
            items.push((*symbol).to_string());
        }
    }

    items.push(if color == Color::BLACK { "☗" } else { "☖" }.to_string());
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;

    #[test]
    fn svg_output_contains_pieces() {
        board::init();
        let mut pos = Position::empty();
        pos.set_hirate();
        let svg = pos.to_svg(None, 1.0);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("xlink:href=\"#black-king\""));
        assert!(svg.contains("xlink:href=\"#white-king\""));
        assert!(svg.contains(">☗<"));
        assert!(svg.contains(">☖<"));
        assert!(!svg.contains(">先<"));
        assert!(!svg.contains(">後<"));
        assert!(!svg.contains(">手<"));
    }
}
