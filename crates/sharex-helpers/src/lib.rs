//! Ports of `ShareX.HelpersLib` that more than one ShareXrs crate needs:
//!
//! - [`name_parser`]: `NameParser`, which turns name patterns such as
//!   ShareX's default file name `%ra{10}` and screenshots sub folder
//!   `%y-%mo` into names.
//! - [`file_helpers`]: `FileHelpers`' file name sanitising and unique paths.
//! - [`random`]: `RandomCrypto` and the `Helpers` character sets.
//! - [`mime_types`]: `MimeTypes`, the MIME type of a file name.
//! - [`cli`]: `CLIManager`, how ShareX reads its command line.
//! - [`single_instance`]: `SingleInstanceManager`, one running copy that
//!   later launches pass their arguments to.
//! - [`culture`]: the .NET string comparisons (current culture through NLS,
//!   ordinal ignoring case) that sorting and matching depend on.

pub mod cli;
pub mod culture;
pub mod date_format;
pub mod dotnet_sort;
pub mod emoji;
pub mod file_helpers;
pub mod mime_types;
pub mod name_parser;
pub mod random;
pub mod single_instance;

pub use name_parser::{NameParser, NameParserType};
