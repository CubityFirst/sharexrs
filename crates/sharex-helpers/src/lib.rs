//! Ports of `ShareX.HelpersLib` that more than one ShareXrs crate needs:
//!
//! - [`name_parser`]: `NameParser`, which turns name patterns such as
//!   ShareX's default file name `%ra{10}` and screenshots sub folder
//!   `%y-%mo` into names.
//! - [`file_helpers`]: `FileHelpers`' file name sanitising and unique paths.
//! - [`random`]: `RandomCrypto` and the `Helpers` character sets.

pub mod emoji;
pub mod file_helpers;
pub mod name_parser;
pub mod random;

pub use name_parser::{NameParser, NameParserType};
