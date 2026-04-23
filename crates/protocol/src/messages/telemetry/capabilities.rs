use serde::{Deserialize, Serialize};

/// Stable controller role announced by a connected firmware image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControllerRole {
    MotionController,
    AudioController,
}

/// Compact capability bitset exported by firmware and interpreted by the host.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControllerCapabilities {
    bits: u32,
}

impl ControllerCapabilities {
    pub const NONE: Self = Self { bits: 0 };
    pub const DRIVE: Self = Self { bits: 1 << 0 };
    pub const SERVO: Self = Self { bits: 1 << 1 };
    pub const RANGE_SENSOR: Self = Self { bits: 1 << 2 };
    pub const BATTERY_MONITOR: Self = Self { bits: 1 << 3 };
    pub const AUDIO_OUTPUT: Self = Self { bits: 1 << 4 };
    pub const TEXT_DISPLAY: Self = Self { bits: 1 << 5 };

    /// Construct a bitset from raw bits.
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Return the raw capability bits.
    pub const fn bits(self) -> u32 {
        self.bits
    }

    /// Return whether this set includes all bits from `other`.
    pub const fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    /// Return a new bitset with `other` added.
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ControllerCapabilities;

    #[test]
    fn capability_bitsets_union_and_query_stably() {
        let capabilities = ControllerCapabilities::DRIVE
            .union(ControllerCapabilities::RANGE_SENSOR)
            .union(ControllerCapabilities::BATTERY_MONITOR);

        assert!(capabilities.contains(ControllerCapabilities::DRIVE));
        assert!(capabilities.contains(ControllerCapabilities::RANGE_SENSOR));
        assert!(!capabilities.contains(ControllerCapabilities::AUDIO_OUTPUT));
        assert_eq!(
            ControllerCapabilities::from_bits(capabilities.bits()),
            capabilities
        );
    }
}