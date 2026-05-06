#![allow(dead_code)]

#[cfg(feature = "sensor-ultrasonic")]
use embassy_rp::gpio::Input;
#[cfg(feature = "driver-l298n")]
use embassy_rp::pwm::Pwm;
use embassy_rp::{
    Peripherals,
    gpio::{Level, Output},
};
use embassy_time::{Delay, Instant};
use mortimmy_core::CoreError;
#[cfg(feature = "ui-display")]
use mortimmy_core::Mode;
#[cfg(feature = "ui-display")]
use mortimmy_drivers::{CharacterDisplay, Hd44780Lcd1602};
#[cfg(feature = "sensor-ultrasonic")]
use mortimmy_drivers::{HcSr04, HcSr04Error, MicrosecondClock, UltrasonicSensor};
#[cfg(feature = "driver-l298n")]
use mortimmy_drivers::{L298nBridge, L298nDriveMotorDriver, L298nSideDriver, MotorDriver};
use mortimmy_protocol::messages::telemetry::{RangeSensorPosition, RangeTelemetry};

#[cfg(not(feature = "driver-l298n"))]
use crate::runtime::NoopRuntimeHardware;
#[cfg(feature = "ui-display")]
use crate::ui::display::{DisplayFrame, render_motion_controller_frame};
use crate::{
    FirmwareScaffold,
    runtime::{BoardRuntime, RuntimeHardware},
};

const MOTION_USB_PRODUCT: &str = "mortimmy motion controller";
const MOTION_USB_SERIAL: &str = "mortimmy-motion-controller";

#[cfg(feature = "ui-display")]
type MotionLcd = Hd44780Lcd1602<
    Output<'static>,
    Output<'static>,
    Output<'static>,
    Output<'static>,
    Output<'static>,
    Output<'static>,
    Delay,
>;

#[cfg(feature = "sensor-ultrasonic")]
const ULTRASONIC_POLL_INTERVAL_US: u32 = 60_000;
#[cfg(feature = "sensor-ultrasonic")]
const ULTRASONIC_LOG_DELTA_MM: u16 = 100;

#[cfg(feature = "sensor-ultrasonic")]
#[derive(Clone, Copy, Debug, Default)]
struct EmbassyMicrosecondClock;

#[cfg(feature = "sensor-ultrasonic")]
impl MicrosecondClock for EmbassyMicrosecondClock {
    fn now_micros(&mut self) -> u32 {
        Instant::now().as_micros() as u32
    }
}

#[cfg(feature = "driver-l298n")]
struct MotionDriveHardware<Driver, Ultrasonic = ()> {
    driver: Driver,
    #[cfg(feature = "sensor-ultrasonic")]
    ultrasonic: Ultrasonic,
    #[cfg(feature = "sensor-ultrasonic")]
    last_ultrasonic_poll_micros: u32,
    #[cfg(feature = "ui-display")]
    display: MotionLcd,
    #[cfg(feature = "ui-display")]
    last_display_frame: Option<DisplayFrame>,
}

#[cfg(feature = "driver-l298n")]
impl<Driver, Ultrasonic> MotionDriveHardware<Driver, Ultrasonic> {
    fn new(
        driver: Driver,
        #[cfg(feature = "sensor-ultrasonic")] ultrasonic: Ultrasonic,
        #[cfg(feature = "ui-display")] display: MotionLcd,
    ) -> Self {
        Self {
            driver,
            #[cfg(feature = "sensor-ultrasonic")]
            ultrasonic,
            #[cfg(feature = "sensor-ultrasonic")]
            last_ultrasonic_poll_micros: 0,
            #[cfg(feature = "ui-display")]
            display,
            #[cfg(feature = "ui-display")]
            last_display_frame: None,
        }
    }

    #[cfg(feature = "ui-display")]
    fn sync_display(&mut self, scaffold: &FirmwareScaffold) {
        let frame = render_motion_controller_frame(scaffold.control.mode);

        if self.last_display_frame.as_ref() == Some(&frame) {
            return;
        }

        let _ = self.display.write_line(0, frame.line0.as_str());
        let _ = self.display.write_line(1, frame.line1.as_str());
        self.last_display_frame = Some(frame);
    }
}

#[cfg(all(feature = "driver-l298n", not(feature = "sensor-ultrasonic")))]
impl<Driver> RuntimeHardware for MotionDriveHardware<Driver>
where
    Driver: MotorDriver,
{
    fn sync_with_scaffold(&mut self, scaffold: &mut FirmwareScaffold) -> Result<(), ()> {
        if scaffold
            .control
            .drive
            .apply_to_driver(&mut self.driver, scaffold.control.limits.max_drive_pwm)
            .is_err()
        {
            defmt::warn!("motion drive hardware sync failed");
            self.enter_fault_state(scaffold, Some(CoreError::InvalidCommand));
            return Err(());
        }

        #[cfg(feature = "ui-display")]
        self.sync_display(scaffold);

        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);

        if self.driver.stop_all().is_err() {
            defmt::warn!("motion drive hardware stop failed");
        }

        #[cfg(feature = "ui-display")]
        self.sync_display(scaffold);
    }
}

#[cfg(all(feature = "driver-l298n", feature = "sensor-ultrasonic"))]
impl<Driver, UltrasonicLeft, UltrasonicRight, TriggerError, EchoError>
    MotionDriveHardware<Driver, (UltrasonicLeft, UltrasonicRight)>
where
    Driver: MotorDriver,
    UltrasonicLeft: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
    UltrasonicRight: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
{
    fn poll_ultrasonic(&mut self, scaffold: &mut FirmwareScaffold) {
        let now_micros = Instant::now().as_micros() as u32;
        if now_micros.wrapping_sub(self.last_ultrasonic_poll_micros) < ULTRASONIC_POLL_INTERVAL_US {
            return;
        }

        self.last_ultrasonic_poll_micros = now_micros;
        scaffold.sensors.ultrasonic.enabled = true;

        let (forward_left, forward_right) = &mut self.ultrasonic;
        poll_ultrasonic_sensor(RangeSensorPosition::ForwardLeft, forward_left, scaffold);
        poll_ultrasonic_sensor(RangeSensorPosition::ForwardRight, forward_right, scaffold);
    }
}

#[cfg(feature = "sensor-ultrasonic")]
fn poll_ultrasonic_sensor<Ultrasonic, TriggerError, EchoError>(
    sensor: RangeSensorPosition,
    ultrasonic: &mut Ultrasonic,
    scaffold: &mut FirmwareScaffold,
) where
    Ultrasonic: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
{
    match ultrasonic.measure_range_mm() {
        Ok(distance) => {
            let previous = previous_range_sample(scaffold, sensor);
            let telemetry = scaffold.sensors.record_range(sensor, distance, 100);
            scaffold.link_tx.queue_range_sample(sensor);
            if should_log_range_sample(previous, telemetry) {
                defmt::info!(
                    "ultrasonic sensor={=str} distance_mm={} quality={}",
                    range_sensor_label(sensor),
                    telemetry.distance_mm.0,
                    telemetry.quality,
                );
            }
        }
        Err(HcSr04Error::OutOfRange(distance)) => {
            let previous = previous_range_sample(scaffold, sensor);
            let telemetry = scaffold.sensors.record_range(sensor, distance, 0);
            scaffold.link_tx.queue_range_sample(sensor);
            if should_log_range_sample(previous, telemetry) {
                defmt::warn!(
                    "ultrasonic sensor={=str} out-of-range distance_mm={} quality={}",
                    range_sensor_label(sensor),
                    telemetry.distance_mm.0,
                    telemetry.quality,
                );
            }
        }
        Err(HcSr04Error::EchoStartTimeout | HcSr04Error::EchoEndTimeout) => {}
        Err(HcSr04Error::Trigger(_) | HcSr04Error::Echo(_)) => {
            defmt::warn!(
                "ultrasonic sensor={=str} gpio read/write failed",
                range_sensor_label(sensor)
            );
        }
    }
}

#[cfg(feature = "sensor-ultrasonic")]
fn previous_range_sample(
    scaffold: &FirmwareScaffold,
    sensor: RangeSensorPosition,
) -> Option<RangeTelemetry> {
    match sensor {
        RangeSensorPosition::ForwardLeft => scaffold.sensors.ultrasonic.ranges.forward_left,
        RangeSensorPosition::ForwardRight => scaffold.sensors.ultrasonic.ranges.forward_right,
    }
}

#[cfg(feature = "sensor-ultrasonic")]
const fn range_sensor_label(sensor: RangeSensorPosition) -> &'static str {
    match sensor {
        RangeSensorPosition::ForwardLeft => "forward-left",
        RangeSensorPosition::ForwardRight => "forward-right",
    }
}

#[cfg(feature = "sensor-ultrasonic")]
fn should_log_range_sample(previous: Option<RangeTelemetry>, current: RangeTelemetry) -> bool {
    match previous {
        None => true,
        Some(previous) => {
            previous.quality != current.quality
                || previous.distance_mm.0.abs_diff(current.distance_mm.0) >= ULTRASONIC_LOG_DELTA_MM
        }
    }
}

#[cfg(all(feature = "driver-l298n", feature = "sensor-ultrasonic"))]
impl<Driver, UltrasonicLeft, UltrasonicRight, TriggerError, EchoError> RuntimeHardware
    for MotionDriveHardware<Driver, (UltrasonicLeft, UltrasonicRight)>
where
    Driver: MotorDriver,
    UltrasonicLeft: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
    UltrasonicRight: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
{
    fn sync_with_scaffold(&mut self, scaffold: &mut FirmwareScaffold) -> Result<(), ()> {
        if scaffold
            .control
            .drive
            .apply_to_driver(&mut self.driver, scaffold.control.limits.max_drive_pwm)
            .is_err()
        {
            defmt::warn!("motion drive hardware sync failed");
            self.enter_fault_state(scaffold, Some(CoreError::InvalidCommand));
            return Err(());
        }

        self.poll_ultrasonic(scaffold);
        #[cfg(feature = "ui-display")]
        self.sync_display(scaffold);
        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);
        self.last_ultrasonic_poll_micros = 0;

        if self.driver.stop_all().is_err() {
            defmt::warn!("motion drive hardware stop failed");
        }

        #[cfg(feature = "ui-display")]
        self.sync_display(scaffold);
    }
}

pub fn build_runtime(
    peripherals: Peripherals,
) -> BoardRuntime<Output<'static>, impl RuntimeHardware> {
    let boot_marker = Output::new(peripherals.PIN_25, Level::Low);

    #[cfg(feature = "ui-display")]
    let mut display = Hd44780Lcd1602::new(
        Output::new(peripherals.PIN_20, Level::Low),
        Output::new(peripherals.PIN_21, Level::Low),
        Output::new(peripherals.PIN_22, Level::Low),
        Output::new(peripherals.PIN_26, Level::Low),
        Output::new(peripherals.PIN_27, Level::Low),
        Output::new(peripherals.PIN_28, Level::Low),
        Delay,
    );
    #[cfg(feature = "ui-display")]
    let boot_frame = {
        let _ = display.initialize();
        let boot_frame = render_motion_controller_frame(Mode::Teleop);
        let _ = display.write_line(0, boot_frame.line0.as_str());
        let _ = display.write_line(1, boot_frame.line1.as_str());
        boot_frame
    };

    #[cfg(all(feature = "driver-l298n", feature = "sensor-ultrasonic"))]
    let mut hardware = MotionDriveHardware::new(
        L298nDriveMotorDriver::new(
            L298nSideDriver::new(
                L298nBridge::new(
                    Output::new(peripherals.PIN_3, Level::Low),
                    Output::new(peripherals.PIN_4, Level::Low),
                    Pwm::new_output_a(
                        peripherals.PWM_SLICE1,
                        peripherals.PIN_2,
                        Default::default(),
                    ),
                ),
                L298nBridge::new(
                    Output::new(peripherals.PIN_5, Level::Low),
                    Output::new(peripherals.PIN_6, Level::Low),
                    Pwm::new_output_b(
                        peripherals.PWM_SLICE3,
                        peripherals.PIN_7,
                        Default::default(),
                    ),
                ),
            ),
            L298nSideDriver::new(
                L298nBridge::new(
                    Output::new(peripherals.PIN_9, Level::Low),
                    Output::new(peripherals.PIN_10, Level::Low),
                    Pwm::new_output_a(
                        peripherals.PWM_SLICE4,
                        peripherals.PIN_8,
                        Default::default(),
                    ),
                ),
                L298nBridge::new(
                    Output::new(peripherals.PIN_11, Level::Low),
                    Output::new(peripherals.PIN_12, Level::Low),
                    Pwm::new_output_b(
                        peripherals.PWM_SLICE6,
                        peripherals.PIN_13,
                        Default::default(),
                    ),
                ),
            ),
        ),
        (
            HcSr04::new(
                Output::new(peripherals.PIN_14, Level::Low),
                Input::new(peripherals.PIN_15, embassy_rp::gpio::Pull::None),
                Delay,
                EmbassyMicrosecondClock,
            ),
            HcSr04::new(
                Output::new(peripherals.PIN_16, Level::Low),
                Input::new(peripherals.PIN_17, embassy_rp::gpio::Pull::None),
                Delay,
                EmbassyMicrosecondClock,
            ),
        ),
        #[cfg(feature = "ui-display")]
        display,
    );

    #[cfg(all(feature = "driver-l298n", not(feature = "sensor-ultrasonic")))]
    let mut hardware = MotionDriveHardware::new(
        L298nDriveMotorDriver::new(
            L298nSideDriver::new(
                L298nBridge::new(
                    Output::new(peripherals.PIN_3, Level::Low),
                    Output::new(peripherals.PIN_4, Level::Low),
                    Pwm::new_output_a(
                        peripherals.PWM_SLICE1,
                        peripherals.PIN_2,
                        Default::default(),
                    ),
                ),
                L298nBridge::new(
                    Output::new(peripherals.PIN_5, Level::Low),
                    Output::new(peripherals.PIN_6, Level::Low),
                    Pwm::new_output_b(
                        peripherals.PWM_SLICE3,
                        peripherals.PIN_7,
                        Default::default(),
                    ),
                ),
            ),
            L298nSideDriver::new(
                L298nBridge::new(
                    Output::new(peripherals.PIN_9, Level::Low),
                    Output::new(peripherals.PIN_10, Level::Low),
                    Pwm::new_output_a(
                        peripherals.PWM_SLICE4,
                        peripherals.PIN_8,
                        Default::default(),
                    ),
                ),
                L298nBridge::new(
                    Output::new(peripherals.PIN_11, Level::Low),
                    Output::new(peripherals.PIN_12, Level::Low),
                    Pwm::new_output_b(
                        peripherals.PWM_SLICE6,
                        peripherals.PIN_13,
                        Default::default(),
                    ),
                ),
            ),
        ),
        #[cfg(feature = "ui-display")]
        display,
    );

    #[cfg(not(feature = "driver-l298n"))]
    let hardware = NoopRuntimeHardware;

    #[cfg(all(feature = "driver-l298n", feature = "ui-display"))]
    {
        hardware.last_display_frame = Some(boot_frame);
    }

    BoardRuntime {
        usb: peripherals.USB,
        boot_marker,
        hardware,
        usb_product: MOTION_USB_PRODUCT,
        usb_serial_number: MOTION_USB_SERIAL,
    }
}
