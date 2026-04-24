use embedded_hal::{delay::DelayNs, digital::OutputPin};

use super::CharacterDisplay;

const DISPLAY_WIDTH: usize = 16;
const CLEAR_FILL: u8 = b' ';

/// HD44780 4-bit timing configuration for a 1602 display.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hd44780Config {
    /// Delay after power-on before initialization begins.
    pub power_on_delay_ms: u32,
    /// Enable pulse width.
    pub enable_pulse_us: u32,
    /// Delay after a regular command or data write.
    pub write_delay_us: u32,
    /// Delay after clear/home commands.
    pub clear_delay_us: u32,
}

impl Default for Hd44780Config {
    fn default() -> Self {
        Self {
            power_on_delay_ms: 15,
            enable_pulse_us: 1,
            write_delay_us: 50,
            clear_delay_us: 2_000,
        }
    }
}

/// Errors returned by the HD44780 display driver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hd44780Error<PinError> {
    Pin(PinError),
    InvalidLine(u8),
}

/// 4-bit write-only HD44780 LCD driver for 16x2 character displays.
#[derive(Debug)]
pub struct Hd44780Lcd1602<Rs, Enable, D4, D5, D6, D7, Delay> {
    rs: Rs,
    enable: Enable,
    d4: D4,
    d5: D5,
    d6: D6,
    d7: D7,
    delay: Delay,
    config: Hd44780Config,
}

impl<Rs, Enable, D4, D5, D6, D7, Delay> Hd44780Lcd1602<Rs, Enable, D4, D5, D6, D7, Delay> {
    /// Construct the display driver with default HD44780 timing.
    pub fn new(rs: Rs, enable: Enable, d4: D4, d5: D5, d6: D6, d7: D7, delay: Delay) -> Self {
        Self::with_config(rs, enable, d4, d5, d6, d7, delay, Hd44780Config::default())
    }

    /// Construct the display driver with explicit timing.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor maps directly onto the HD44780 control and data pins"
    )]
    pub const fn with_config(
        rs: Rs,
        enable: Enable,
        d4: D4,
        d5: D5,
        d6: D6,
        d7: D7,
        delay: Delay,
        config: Hd44780Config,
    ) -> Self {
        Self {
            rs,
            enable,
            d4,
            d5,
            d6,
            d7,
            delay,
            config,
        }
    }

    /// Run the HD44780 4-bit initialization sequence.
    pub fn initialize<PinError>(&mut self) -> Result<(), Hd44780Error<PinError>>
    where
        Rs: OutputPin<Error = PinError>,
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        self.rs.set_low().map_err(Hd44780Error::Pin)?;
        self.enable.set_low().map_err(Hd44780Error::Pin)?;
        self.delay.delay_ms(self.config.power_on_delay_ms);

        self.write_init_nibble(0x03)?;
        self.delay.delay_ms(5);
        self.write_init_nibble(0x03)?;
        self.delay.delay_us(150);
        self.write_init_nibble(0x03)?;
        self.delay.delay_us(150);
        self.write_init_nibble(0x02)?;

        self.write_command(0x28)?;
        self.write_command(0x08)?;
        self.clear()?;
        self.write_command(0x06)?;
        self.write_command(0x0C)
    }

    /// Set the cursor position.
    pub fn set_cursor<PinError>(
        &mut self,
        line: u8,
        column: u8,
    ) -> Result<(), Hd44780Error<PinError>>
    where
        Rs: OutputPin<Error = PinError>,
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        let row_base = match line {
            0 => 0x00,
            1 => 0x40,
            invalid => return Err(Hd44780Error::InvalidLine(invalid)),
        };

        self.write_command(0x80 | (row_base + column.min((DISPLAY_WIDTH - 1) as u8)))
    }

    fn write_command<PinError>(&mut self, command: u8) -> Result<(), Hd44780Error<PinError>>
    where
        Rs: OutputPin<Error = PinError>,
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        self.write_byte(false, command)?;
        self.delay.delay_us(self.config.write_delay_us);
        Ok(())
    }

    fn write_data<PinError>(&mut self, data: u8) -> Result<(), Hd44780Error<PinError>>
    where
        Rs: OutputPin<Error = PinError>,
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        self.write_byte(true, data)?;
        self.delay.delay_us(self.config.write_delay_us);
        Ok(())
    }

    fn write_init_nibble<PinError>(&mut self, nibble: u8) -> Result<(), Hd44780Error<PinError>>
    where
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        self.set_data_nibble(nibble)?;
        self.pulse_enable()
    }

    fn write_byte<PinError>(
        &mut self,
        is_data: bool,
        value: u8,
    ) -> Result<(), Hd44780Error<PinError>>
    where
        Rs: OutputPin<Error = PinError>,
        Enable: OutputPin<Error = PinError>,
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        if is_data {
            self.rs.set_high().map_err(Hd44780Error::Pin)?;
        } else {
            self.rs.set_low().map_err(Hd44780Error::Pin)?;
        }

        self.set_data_nibble(value >> 4)?;
        self.pulse_enable()?;
        self.set_data_nibble(value & 0x0f)?;
        self.pulse_enable()
    }

    fn set_data_nibble<PinError>(&mut self, nibble: u8) -> Result<(), Hd44780Error<PinError>>
    where
        D4: OutputPin<Error = PinError>,
        D5: OutputPin<Error = PinError>,
        D6: OutputPin<Error = PinError>,
        D7: OutputPin<Error = PinError>,
    {
        self.d4
            .set_state((nibble & 0x01 != 0).into())
            .map_err(Hd44780Error::Pin)?;
        self.d5
            .set_state(((nibble >> 1) & 0x01 != 0).into())
            .map_err(Hd44780Error::Pin)?;
        self.d6
            .set_state(((nibble >> 2) & 0x01 != 0).into())
            .map_err(Hd44780Error::Pin)?;
        self.d7
            .set_state(((nibble >> 3) & 0x01 != 0).into())
            .map_err(Hd44780Error::Pin)
    }

    fn pulse_enable<PinError>(&mut self) -> Result<(), Hd44780Error<PinError>>
    where
        Enable: OutputPin<Error = PinError>,
        Delay: DelayNs,
    {
        self.enable.set_high().map_err(Hd44780Error::Pin)?;
        self.delay.delay_us(self.config.enable_pulse_us);
        self.enable.set_low().map_err(Hd44780Error::Pin)
    }
}

impl<Rs, Enable, D4, D5, D6, D7, Delay, PinError> CharacterDisplay
    for Hd44780Lcd1602<Rs, Enable, D4, D5, D6, D7, Delay>
where
    Rs: OutputPin<Error = PinError>,
    Enable: OutputPin<Error = PinError>,
    D4: OutputPin<Error = PinError>,
    D5: OutputPin<Error = PinError>,
    D6: OutputPin<Error = PinError>,
    D7: OutputPin<Error = PinError>,
    Delay: DelayNs,
{
    type Error = Hd44780Error<PinError>;

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.write_byte(false, 0x01)?;
        self.delay.delay_us(self.config.clear_delay_us);
        Ok(())
    }

    fn write_line(&mut self, line: u8, text: &str) -> Result<(), Self::Error> {
        self.set_cursor(line, 0)?;

        let mut written = 0usize;
        for byte in text.bytes().take(DISPLAY_WIDTH) {
            self.write_data(byte)?;
            written += 1;
        }

        while written < DISPLAY_WIDTH {
            self.write_data(CLEAR_FILL)?;
            written += 1;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{cell::RefCell, rc::Rc, vec::Vec};

    use embedded_hal::{
        delay::DelayNs,
        digital::{ErrorType, OutputPin},
    };

    use super::Hd44780Lcd1602;
    use crate::ui::display::CharacterDisplay;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct FakePinError;

    impl embedded_hal::digital::Error for FakePinError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    #[derive(Debug, Default)]
    struct BusCapture {
        rs: bool,
        data: [bool; 4],
        enable_high: bool,
        nibbles: Vec<(bool, u8)>,
    }

    #[derive(Clone, Debug)]
    struct RsPin {
        bus: Rc<RefCell<BusCapture>>,
    }

    #[derive(Clone, Debug)]
    struct DataPin {
        bus: Rc<RefCell<BusCapture>>,
        index: usize,
    }

    #[derive(Clone, Debug)]
    struct EnablePin {
        bus: Rc<RefCell<BusCapture>>,
    }

    impl ErrorType for RsPin {
        type Error = FakePinError;
    }

    impl ErrorType for DataPin {
        type Error = FakePinError;
    }

    impl ErrorType for EnablePin {
        type Error = FakePinError;
    }

    impl OutputPin for RsPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.bus.borrow_mut().rs = false;
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.bus.borrow_mut().rs = true;
            Ok(())
        }
    }

    impl OutputPin for DataPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.bus.borrow_mut().data[self.index] = false;
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.bus.borrow_mut().data[self.index] = true;
            Ok(())
        }
    }

    impl OutputPin for EnablePin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            let mut bus = self.bus.borrow_mut();
            if bus.enable_high {
                let nibble = u8::from(bus.data[0])
                    | (u8::from(bus.data[1]) << 1)
                    | (u8::from(bus.data[2]) << 2)
                    | (u8::from(bus.data[3]) << 3);
                let rs = bus.rs;
                bus.nibbles.push((rs, nibble));
            }
            bus.enable_high = false;
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.bus.borrow_mut().enable_high = true;
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct FakeDelay;

    impl DelayNs for FakeDelay {
        fn delay_ns(&mut self, _ns: u32) {}
    }

    type TestLcd = Hd44780Lcd1602<RsPin, EnablePin, DataPin, DataPin, DataPin, DataPin, FakeDelay>;
    type SharedBus = Rc<RefCell<BusCapture>>;

    fn build_lcd() -> (TestLcd, SharedBus) {
        let bus = Rc::new(RefCell::new(BusCapture::default()));
        (
            Hd44780Lcd1602::new(
                RsPin { bus: bus.clone() },
                EnablePin { bus: bus.clone() },
                DataPin {
                    bus: bus.clone(),
                    index: 0,
                },
                DataPin {
                    bus: bus.clone(),
                    index: 1,
                },
                DataPin {
                    bus: bus.clone(),
                    index: 2,
                },
                DataPin {
                    bus: bus.clone(),
                    index: 3,
                },
                FakeDelay,
            ),
            bus,
        )
    }

    fn decode_bytes(nibbles: &[(bool, u8)]) -> Vec<(bool, u8)> {
        nibbles
            .chunks_exact(2)
            .map(|pair| (pair[0].0, (pair[0].1 << 4) | pair[1].1))
            .collect()
    }

    #[test]
    fn hd44780_initialization_emits_standard_4_bit_sequence() {
        let (mut lcd, bus) = build_lcd();

        lcd.initialize().unwrap();

        let bus = bus.borrow();
        assert_eq!(
            &bus.nibbles[..4],
            &[(false, 0x03), (false, 0x03), (false, 0x03), (false, 0x02)]
        );

        let bytes = decode_bytes(&bus.nibbles[4..]);
        assert!(bytes.contains(&(false, 0x28)));
        assert!(bytes.contains(&(false, 0x01)));
        assert!(bytes.contains(&(false, 0x0c)));
    }

    #[test]
    fn hd44780_write_line_targets_second_row_and_pads_spaces() {
        let (mut lcd, bus) = build_lcd();
        lcd.initialize().unwrap();

        bus.borrow_mut().nibbles.clear();
        lcd.write_line(1, "OK").unwrap();

        let bytes = decode_bytes(&bus.borrow().nibbles);
        assert_eq!(bytes[0], (false, 0xc0));
        assert_eq!(bytes[1], (true, b'O'));
        assert_eq!(bytes[2], (true, b'K'));
        assert_eq!(bytes[3], (true, b' '));
        assert_eq!(bytes.len(), 17);
    }
}
