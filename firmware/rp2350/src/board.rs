//! Board profile definitions for the currently supported embedded hardware.

/// Static description of a supported embedded board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardProfile {
    /// Human-readable board name.
    pub name: &'static str,
    /// Microcontroller on the board.
    pub mcu: &'static str,
    /// External flash capacity in bytes.
    pub flash_bytes: usize,
    /// External PSRAM capacity in bytes.
    pub psram_bytes: usize,
    /// Whether the board exposes USB-C.
    pub has_usb_c: bool,
    /// Whether the board includes a Qw/ST connector.
    pub has_qwst: bool,
    /// Whether the board includes LiPo charging support.
    pub has_lipo_charger: bool,
}

/// Current board target: Pimoroni Pico LiPo 2 with RP2350B.
pub const PIMORONI_PICO_LIPO_2: BoardProfile = BoardProfile {
    name: "Pimoroni Pico LiPo 2",
    mcu: "RP2350B",
    flash_bytes: 16 * 1024 * 1024,
    psram_bytes: 8 * 1024 * 1024,
    has_usb_c: true,
    has_qwst: true,
    has_lipo_charger: true,
};

/// Audio-controller target: Raspberry Pi Pico 2 W with Pico Audio Pack.
pub const RASPBERRY_PI_PICO_2_W: BoardProfile = BoardProfile {
    name: "Pico 2 W + Pico Audio Pack",
    mcu: "RP2350",
    flash_bytes: 4 * 1024 * 1024,
    psram_bytes: 0,
    has_usb_c: false,
    has_qwst: false,
    has_lipo_charger: false,
};

/// Return the compile-time selected board profile.
pub const fn active_board_profile() -> BoardProfile {
    if cfg!(feature = "board-audio-controller") {
        RASPBERRY_PI_PICO_2_W
    } else {
        PIMORONI_PICO_LIPO_2
    }
}
