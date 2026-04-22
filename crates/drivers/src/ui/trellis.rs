//! Traits and helper types for Trellis-style keypad and LED matrix devices.

/// Index of a pad in a 4x4 matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadIndex(u8);

impl PadIndex {
    /// Number of pads available on the 4x4 Trellis matrix.
    pub const PAD_COUNT: u8 = 16;

    /// Create a validated pad index.
    pub const fn new(index: u8) -> Option<Self> {
        if index < Self::PAD_COUNT {
            Some(Self(index))
        } else {
            None
        }
    }

    /// Return the zero-based pad index.
    pub const fn as_u8(self) -> u8 {
        self.0
    }
}

/// State transition reported by a Trellis pad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadEventKind {
    /// The pad was pressed.
    Pressed,
    /// The pad was released.
    Released,
}

/// Event emitted by a Trellis pad matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadEvent {
    /// The affected pad.
    pub index: PadIndex,
    /// The pad transition.
    pub kind: PadEventKind,
}

/// Trait implemented by keypad and LED matrix drivers.
pub trait TrellisPadMatrix {
    /// Driver-specific error type.
    type Error;

    /// Update the 16-bit LED mask, where each bit maps to a pad LED.
    fn set_led_mask(&mut self, mask: u16) -> Result<(), Self::Error>;

    /// Poll the hardware for the next keypad event.
    fn poll_event(&mut self) -> Result<Option<PadEvent>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::PadIndex;

    #[test]
    fn accepts_valid_pad_index() {
        assert_eq!(PadIndex::new(0).map(PadIndex::as_u8), Some(0));
        assert_eq!(PadIndex::new(15).map(PadIndex::as_u8), Some(15));
    }

    #[test]
    fn rejects_out_of_range_pad_index() {
        assert_eq!(PadIndex::new(16), None);
        assert_eq!(PadIndex::new(u8::MAX), None);
    }
}