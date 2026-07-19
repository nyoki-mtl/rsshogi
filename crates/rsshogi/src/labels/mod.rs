//! 学習や推論で利用する指し手ラベル変換。
//!
//! 現在は policy 学習向けの 27x81 move label scheme を提供する。
//! ここでは局面やテンソル表現に依存しない、純粋なラベル変換のみを扱う。
//! NumPy / TensorFlow 向けの gather index など、モデル実装依存の補助は含めない。

pub mod policy;
