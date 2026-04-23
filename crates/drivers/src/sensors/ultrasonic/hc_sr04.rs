use embedded_hal::{
    delay::DelayNs,
    digital::{InputPin, OutputPin},
};
use mortimmy_core::Millimeters;

use super::UltrasonicSensor;

/// Monotonic microsecond clock used to time HC-SR04 echo pulses.
pub trait MicrosecondClock {
    /// Return the current monotonic time in microseconds.
    fn now_micros(&mut self) -> u32;
}

/// Tunable HC-SR04 timing and conversion parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HcSr04Config {
    /// Time to hold the trigger pin low before beginning a measurement.
    pub pre_trigger_settle_us: u32,
    /// Trigger pulse width.
    pub trigger_pulse_us: u32,
    /// Maximum wait for the echo pulse to start.
    pub echo_start_timeout_us: u32,
    /// Maximum wait for the echo pulse to end.
    pub echo_end_timeout_us: u32,
    /// Minimum accepted range.
    pub min_distance_mm: u16,
    /// Maximum accepted range.
    pub max_distance_mm: u16,
    /// Speed of sound used for distance conversion.
    pub sound_speed_mm_per_s: u32,
}

impl Default for HcSr04Config {
    fn default() -> Self {
        Self {
            pre_trigger_settle_us: 2,
            trigger_pulse_us: 10,
            echo_start_timeout_us: 1_000,
            echo_end_timeout_us: 25_000,
            min_distance_mm: 20,
            max_distance_mm: 4_000,
            sound_speed_mm_per_s: 343_000,
        }
    }
}

/// HC-SR04 driver failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HcSr04Error<TriggerError, EchoError> {
    Trigger(TriggerError),
    Echo(EchoError),
    EchoStartTimeout,
    EchoEndTimeout,
    OutOfRange(Millimeters),
}

/// Blocking HC-SR04 driver using one trigger output, one echo input, a delay, and a clock.
#[derive(Debug)]
pub struct HcSr04<Trigger, Echo, Delay, Clock> {
    trigger: Trigger,
    echo: Echo,
    delay: Delay,
    clock: Clock,
    config: HcSr04Config,
}

impl<Trigger, Echo, Delay, Clock> HcSr04<Trigger, Echo, Delay, Clock> {
    /// Construct the driver with default HC-SR04 timing.
    pub fn new(trigger: Trigger, echo: Echo, delay: Delay, clock: Clock) -> Self {
        Self::with_config(trigger, echo, delay, clock, HcSr04Config::default())
    }

    /// Construct the driver with explicit timing parameters.
    pub const fn with_config(
        trigger: Trigger,
        echo: Echo,
        delay: Delay,
        clock: Clock,
        config: HcSr04Config,
    ) -> Self {
        Self {
            trigger,
            echo,
            delay,
            clock,
            config,
        }
    }

    /// Access the configured timing values.
    pub const fn config(&self) -> HcSr04Config {
        self.config
    }

    fn wait_for_echo_level<TriggerError, EchoError>(
        &mut self,
        expected_high: bool,
        timeout_us: u32,
    ) -> Result<u32, HcSr04Error<TriggerError, EchoError>>
    where
        Trigger: OutputPin<Error = TriggerError>,
        Echo: InputPin<Error = EchoError>,
        Delay: DelayNs,
        Clock: MicrosecondClock,
    {
        let started_at = self.clock.now_micros();

        loop {
            let is_high = self.echo.is_high().map_err(HcSr04Error::Echo)?;
            if is_high == expected_high {
                return Ok(self.clock.now_micros());
            }

            if elapsed_micros(started_at, self.clock.now_micros()) >= timeout_us {
                return Err(if expected_high {
                    HcSr04Error::EchoStartTimeout
                } else {
                    HcSr04Error::EchoEndTimeout
                });
            }

            self.delay.delay_us(1);
        }
    }

    fn distance_for_pulse_us(&self, pulse_width_us: u32) -> Millimeters {
        let scaled_distance = (u64::from(pulse_width_us)
            * u64::from(self.config.sound_speed_mm_per_s)
            + 1_000_000)
            / 2_000_000;
        Millimeters(scaled_distance.min(u64::from(u16::MAX)) as u16)
    }
}

impl<Trigger, Echo, Delay, Clock, TriggerError, EchoError> UltrasonicSensor
    for HcSr04<Trigger, Echo, Delay, Clock>
where
    Trigger: OutputPin<Error = TriggerError>,
    Echo: InputPin<Error = EchoError>,
    Delay: DelayNs,
    Clock: MicrosecondClock,
{
    type Error = HcSr04Error<TriggerError, EchoError>;

    fn measure_range_mm(&mut self) -> Result<Millimeters, Self::Error> {
        self.trigger.set_low().map_err(HcSr04Error::Trigger)?;
        self.delay.delay_us(self.config.pre_trigger_settle_us);
        self.trigger.set_high().map_err(HcSr04Error::Trigger)?;
        self.delay.delay_us(self.config.trigger_pulse_us);
        self.trigger.set_low().map_err(HcSr04Error::Trigger)?;

        let pulse_started =
            self.wait_for_echo_level(true, self.config.echo_start_timeout_us)?;
        let pulse_ended = self.wait_for_echo_level(false, self.config.echo_end_timeout_us)?;
        let distance = self.distance_for_pulse_us(elapsed_micros(pulse_started, pulse_ended));

        if distance.0 < self.config.min_distance_mm || distance.0 > self.config.max_distance_mm {
            return Err(HcSr04Error::OutOfRange(distance));
        }

        Ok(distance)
    }
}

const fn elapsed_micros(started_at: u32, now: u32) -> u32 {
    now.wrapping_sub(started_at)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
        vec::Vec,
    };

    use embedded_hal::{
        delay::DelayNs,
        digital::{ErrorType, InputPin, OutputPin},
    };

    use super::{HcSr04, HcSr04Error, MicrosecondClock};
    use crate::sensors::ultrasonic::UltrasonicSensor;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum FakePinError {
        Injected,
    }

    impl embedded_hal::digital::Error for FakePinError {
        fn kind(&self) -> embedded_hal::digital::ErrorKind {
            embedded_hal::digital::ErrorKind::Other
        }
    }

    #[derive(Clone, Debug)]
    struct FakeClock {
        now_us: Rc<Cell<u32>>,
    }

    impl MicrosecondClock for FakeClock {
        fn now_micros(&mut self) -> u32 {
            self.now_us.get()
        }
    }

    #[derive(Clone, Debug)]
    struct FakeDelay {
        now_us: Rc<Cell<u32>>,
    }

    impl DelayNs for FakeDelay {
        fn delay_ns(&mut self, ns: u32) {
            let rounded_up_us = (ns.saturating_add(999)) / 1_000;
            self.now_us
                .set(self.now_us.get().saturating_add(rounded_up_us.max(1)));
        }
    }

    #[derive(Clone, Debug)]
    struct FakeTriggerPin {
        states: Rc<RefCell<Vec<bool>>>,
    }

    impl ErrorType for FakeTriggerPin {
        type Error = FakePinError;
    }

    impl OutputPin for FakeTriggerPin {
        fn set_low(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(false);
            Ok(())
        }

        fn set_high(&mut self) -> Result<(), Self::Error> {
            self.states.borrow_mut().push(true);
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    struct ScriptedEchoPin {
        now_us: Rc<Cell<u32>>,
        pulse_start_us: u32,
        pulse_end_us: u32,
        fail: bool,
    }

    impl ErrorType for ScriptedEchoPin {
        type Error = FakePinError;
    }

    impl InputPin for ScriptedEchoPin {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            if self.fail {
                return Err(FakePinError::Injected);
            }

            let now = self.now_us.get();
            Ok(now >= self.pulse_start_us && now < self.pulse_end_us)
        }

        fn is_low(&mut self) -> Result<bool, Self::Error> {
            self.is_high().map(|is_high| !is_high)
        }
    }

    #[test]
    fn hc_sr04_measures_distance_from_echo_pulse() {
        let now_us = Rc::new(Cell::new(0));
        let trigger_states = Rc::new(RefCell::new(Vec::new()));
        let mut sensor = HcSr04::new(
            FakeTriggerPin {
                states: trigger_states.clone(),
            },
            ScriptedEchoPin {
                now_us: now_us.clone(),
                pulse_start_us: 30,
                pulse_end_us: 1_030,
                fail: false,
            },
            FakeDelay {
                now_us: now_us.clone(),
            },
            FakeClock {
                now_us: now_us.clone(),
            },
        );

        let distance = sensor.measure_range_mm().unwrap();

        assert_eq!(distance.0, 172);
        assert_eq!(&*trigger_states.borrow(), &[false, true, false]);
    }

    #[test]
    fn hc_sr04_times_out_when_echo_never_starts() {
        let now_us = Rc::new(Cell::new(0));
        let mut sensor = HcSr04::new(
            FakeTriggerPin {
                states: Rc::new(RefCell::new(Vec::new())),
            },
            ScriptedEchoPin {
                now_us: now_us.clone(),
                pulse_start_us: 5_000,
                pulse_end_us: 6_000,
                fail: false,
            },
            FakeDelay {
                now_us: now_us.clone(),
            },
            FakeClock {
                now_us: now_us.clone(),
            },
        );

        assert_eq!(
            sensor.measure_range_mm(),
            Err(HcSr04Error::EchoStartTimeout)
        );
    }

    #[test]
    fn hc_sr04_rejects_ranges_outside_sensor_envelope() {
        let now_us = Rc::new(Cell::new(0));
        let mut sensor = HcSr04::new(
            FakeTriggerPin {
                states: Rc::new(RefCell::new(Vec::new())),
            },
            ScriptedEchoPin {
                now_us: now_us.clone(),
                pulse_start_us: 30,
                pulse_end_us: 24_030,
                fail: false,
            },
            FakeDelay {
                now_us: now_us.clone(),
            },
            FakeClock { now_us },
        );

        assert_eq!(
            sensor.measure_range_mm(),
            Err(HcSr04Error::OutOfRange(mortimmy_core::Millimeters(4116)))
        );
    }
}
