//! Character display traits and HD44780-compatible implementations.

pub mod hd44780;

/// Trait implemented by character displays used for local status output.
pub trait CharacterDisplay {
    /// Driver-specific error type.
    type Error;

    /// Clear the display contents.
    fn clear(&mut self) -> Result<(), Self::Error>;

    /// Write one logical line, truncating or padding to the display width.
    fn write_line(&mut self, line: u8, text: &str) -> Result<(), Self::Error>;
}
