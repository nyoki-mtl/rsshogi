//! テーブル駆動版 1 手詰め判定用の事前計算テーブル
//!
//! 玉の周囲 8 方向の利き状況を 16 ビットのインデックスにエンコードし、
//! そのインデックスから「どの持ち駒で詰むか」「どの方向から詰むか」を
//! テーブル引きで判定する。

#![allow(long_running_const_eval)]

/// 方向を表すビットマスク。
pub type Directions = u8;

/// 1 手詰めテーブルのエントリ。
#[derive(Clone, Copy, Default)]
pub struct MateInfo {
    /// 詰みに使える持ち駒種のビットマスク
    pub hand_kind: u8,
    /// 詰みとなる方向のビットマスク
    pub directions: Directions,
}

/// テーブルのサイズ（16 ビットインデックス = 65,536 エントリ）。
pub const MATE_INFO_TABLE_SIZE: usize = 0x10000;

use std::sync::OnceLock;

static MATE_TBL: OnceLock<Box<[[MateInfo; 2]; MATE_INFO_TABLE_SIZE]>> = OnceLock::new();

pub fn mate_info(idx: usize, c: usize) -> MateInfo {
    MATE_TBL.get_or_init(init_mate1ply_tbl)[idx][c]
}

// テーブル生成用の駒種定数
const T_PAWN: usize = 0;
const T_LANCE: usize = 1;
const T_KNIGHT: usize = 2;
const T_SILVER: usize = 3;
const T_BISHOP: usize = 4;
const T_ROOK: usize = 5;
const T_GOLD: usize = 6;
const T_QUEEN: usize = 7;

// 未使用警告対策は不要。init_mate1ply_tbl 内で使用している。

const DIRS_AROUND8: [(i32, i32); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

/// `piece_idx` が攻撃する玉周囲 8 近傍のビットマスクを返す。
fn attack_mask_with_dirs(
    piece_idx: usize,
    msg_dir: usize,
    color: usize,
    dirs: &[(i32, i32); 8],
) -> u8 {
    let (px, py) = dirs[msg_dir];

    let mut mask = 0u8;
    for (i, &(kx, ky)) in dirs.iter().enumerate() {
        let dx: i32 = kx - px;
        let dy: i32 = ky - py;

        if check_attack(piece_idx, dx, dy, color) {
            mask |= 1 << i;
        }
    }
    mask
}

const fn check_attack(pt: usize, dx: i32, dy: i32, color: usize) -> bool {
    match pt {
        T_PAWN => {
            if color == 0 {
                dx == 0 && dy == -1
            } else {
                dx == 0 && dy == 1
            }
        }
        T_LANCE => {
            if color == 0 {
                dx == 0 && dy < 0
            } else {
                dx == 0 && dy > 0
            }
        }
        T_KNIGHT => {
            if color == 0 {
                dx.abs() == 1 && dy == -2
            } else {
                dx.abs() == 1 && dy == 2
            }
        }
        T_SILVER => {
            if color == 0 {
                (dx.abs() <= 1 && dy.abs() <= 1)
                    && !(dx == 0 && dy == 1)
                    && !(dx.abs() == 1 && dy == 0)
            } else {
                (dx.abs() <= 1 && dy.abs() <= 1)
                    && !(dx == 0 && dy == -1)
                    && !(dx.abs() == 1 && dy == 0)
            }
        }
        T_GOLD => {
            if color == 0 {
                (dx.abs() <= 1 && dy.abs() <= 1) && !(dx.abs() == 1 && dy == 1)
            } else {
                (dx.abs() <= 1 && dy.abs() <= 1) && !(dx.abs() == 1 && dy == -1)
            }
        }
        T_BISHOP => dx.abs() == dy.abs(),
        T_ROOK => dx == 0 || dy == 0,
        T_QUEEN => dx == 0 || dy == 0 || dx.abs() == dy.abs(),
        _ => false,
    }
}

#[allow(clippy::cast_possible_truncation)]
fn init_mate1ply_tbl_with_dirs(
    dirs: &[(i32, i32); 8],
) -> Box<[[MateInfo; 2]; MATE_INFO_TABLE_SIZE]> {
    let mut vec = vec![[MateInfo::default(); 2]; MATE_INFO_TABLE_SIZE];

    for (info, item) in vec.iter_mut().enumerate() {
        for (c, entry) in item.iter_mut().enumerate().take(2) {
            let info1 = (info & 0xFF) as u8; // 打ち駒候補の利きビット
            let info2 = ((info >> 8) & 0xFF) as u8; // 玉の移動可能方向ビット

            let mut hk = 0u8;
            let mut directions = 0u8;

            for dir in 0..8 {
                let droppable = (info1 & (1 << dir)) != 0;
                let pieces = [T_LANCE, T_SILVER, T_GOLD, T_BISHOP, T_ROOK, T_QUEEN];
                for &pt in &pieces {
                    if !droppable && pt != T_QUEEN {
                        continue;
                    }

                    let attacks = attack_mask_with_dirs(pt, dir, c, dirs);
                    let attacks_not = !attacks;
                    let king_movable = attacks_not & info2;
                    if king_movable == 0 {
                        if pt == T_QUEEN {
                            directions |= 1 << dir;
                        } else {
                            let bit = match pt {
                                T_LANCE => 1,
                                T_SILVER => 3,
                                T_BISHOP => 4,
                                T_ROOK => 5,
                                T_GOLD => 6,
                                _ => 0,
                            };
                            if bit > 0 {
                                hk |= 1 << bit;
                            }
                        }
                    }
                }

                if info2 == 0 {
                    hk |= 1 << 2;
                }
            }

            *entry = MateInfo { hand_kind: hk, directions };
        }
    }

    vec.into_boxed_slice().try_into().unwrap_or_else(|_| panic!("size mismatch"))
}

#[allow(clippy::cast_possible_truncation)]
fn init_mate1ply_tbl() -> Box<[[MateInfo; 2]; MATE_INFO_TABLE_SIZE]> {
    init_mate1ply_tbl_with_dirs(&DIRS_AROUND8)
}
