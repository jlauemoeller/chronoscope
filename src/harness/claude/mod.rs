//! The Claude Code adapter (SPEC §3.1, §5.1).

pub mod assemble;
pub mod tools;
mod wire;

pub use assemble::Assembler;
pub(crate) use wire::extract_result_text;
