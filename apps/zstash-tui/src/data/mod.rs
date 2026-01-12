//! Data layer for zkore-tui.
//!
//! Provides read and write operations for wallet data,
//! including encryption utilities for accessing encrypted wallet databases.

pub mod encryption;
pub mod wallet_reader;
pub mod wallet_writer;

pub use encryption::*;
pub use wallet_reader::*;
pub use wallet_writer::*;
