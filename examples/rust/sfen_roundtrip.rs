use rsshogi::board;

fn main() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let pos = board::position_from_sfen(sfen).expect("valid sfen");
    println!("{}", pos.to_sfen(None));
}
