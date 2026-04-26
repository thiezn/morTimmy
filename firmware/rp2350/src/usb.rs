#![allow(dead_code)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_futures::join::join;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_executor::Spawner;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_futures::select::{Either, select};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_rp::gpio::{Level, Output};
#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
use embassy_rp::gpio::Input;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "driver-l298n"))]
use embassy_rp::pwm::Pwm;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_rp::{bind_interrupts, peripherals, usb};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_time::{Duration as EmbassyDuration, Timer};
#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
use embassy_time::{Delay, Instant};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_usb::{Builder, Config, class::cdc_acm::{CdcAcmClass, Receiver, Sender, State as CdcAcmState}, driver::EndpointError};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use mortimmy_core::CoreError;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "driver-l298n"))]
use mortimmy_drivers::{L298nBridge, L298nDriveMotorDriver, L298nSideDriver, MotorDriver};
#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
use mortimmy_drivers::{HcSr04, HcSr04Error, MicrosecondClock, UltrasonicSensor};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use mortimmy_protocol::{FrameDecoder, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message, encode_message, wrap_payload};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use mortimmy_protocol::messages::{WireMessage, telemetry::{RangeTelemetry, Telemetry}};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use static_cell::StaticCell;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::FirmwareScaffold;

#[cfg(all(target_arch = "arm", target_os = "none"))]
type UsbDriver = embassy_rp::usb::Driver<'static, peripherals::USB>;

#[cfg(all(target_arch = "arm", target_os = "none"))]
const USB_VENDOR_ID: u16 = 0x2E8A;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const USB_PRODUCT_ID: u16 = 0x000A;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const USB_MAX_PACKET_SIZE: usize = 64;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const BOOT_BLINK_DELAY_CYCLES: u32 = 60_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_BLINK_DELAY_CYCLES: u32 = 30_000_000;
#[cfg(all(target_arch = "arm", target_os = "none"))]
const STAGE_BLINK_GAP_CYCLES: u32 = 90_000_000;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
const ULTRASONIC_POLL_INTERVAL_US: u32 = 100_000;
#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
const ULTRASONIC_LOG_DELTA_MM: u16 = 100;

#[cfg(all(target_arch = "arm", target_os = "none"))]
bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<peripherals::USB>;
});

#[cfg(all(target_arch = "arm", target_os = "none"))]
static CDC_STATE: StaticCell<CdcAcmState<'static>> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CONTROL_BUFFER: StaticCell<[u8; 64]> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CDC_SENDER: StaticCell<Sender<'static, UsbDriver>> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static CDC_RECEIVER: StaticCell<Receiver<'static, UsbDriver>> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static FRAME_DECODER: StaticCell<FrameDecoder> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static PAYLOAD_BUFFER: StaticCell<[u8; MAX_PAYLOAD_LEN]> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static FRAME_BUFFER: StaticCell<[u8; MAX_FRAME_BODY_LEN + 1]> = StaticCell::new();
#[cfg(all(target_arch = "arm", target_os = "none"))]
static FIRMWARE_SCAFFOLD: StaticCell<FirmwareScaffold> = StaticCell::new();

#[cfg(all(target_arch = "arm", target_os = "none"))]
trait RuntimeHardware {
    fn sync_with_scaffold(&mut self, scaffold: &mut FirmwareScaffold) -> Result<(), ()>;

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
struct NoopRuntimeHardware;

#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
#[derive(Clone, Copy, Debug, Default)]
struct EmbassyMicrosecondClock;

#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
impl MicrosecondClock for EmbassyMicrosecondClock {
    fn now_micros(&mut self) -> u32 {
        Instant::now().as_micros() as u32
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RuntimeHardware for NoopRuntimeHardware {
    fn sync_with_scaffold(&mut self, _scaffold: &mut FirmwareScaffold) -> Result<(), ()> {
        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);
    }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "driver-l298n"))]
struct MotionDriveHardware<Driver, Ultrasonic = ()> {
    driver: Driver,
    #[cfg(feature = "sensor-ultrasonic")]
    ultrasonic: Ultrasonic,
    #[cfg(feature = "sensor-ultrasonic")]
    last_ultrasonic_poll_micros: u32,
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "driver-l298n"))]
impl<Driver, Ultrasonic> MotionDriveHardware<Driver, Ultrasonic> {
    fn new(
        driver: Driver,
        #[cfg(feature = "sensor-ultrasonic")] ultrasonic: Ultrasonic,
    ) -> Self {
        Self {
            driver,
            #[cfg(feature = "sensor-ultrasonic")]
            ultrasonic,
            #[cfg(feature = "sensor-ultrasonic")]
            last_ultrasonic_poll_micros: 0,
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "driver-l298n",
    not(feature = "sensor-ultrasonic")
))]
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

        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);

        if self.driver.stop_all().is_err() {
            defmt::warn!("motion drive hardware stop failed");
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "driver-l298n",
    feature = "sensor-ultrasonic"
))]
impl<Driver, Ultrasonic, TriggerError, EchoError> MotionDriveHardware<Driver, Ultrasonic>
where
    Driver: MotorDriver,
    Ultrasonic: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
{
    fn poll_ultrasonic(&mut self, scaffold: &mut FirmwareScaffold) {
        let now_micros = Instant::now().as_micros() as u32;
        if now_micros.wrapping_sub(self.last_ultrasonic_poll_micros) < ULTRASONIC_POLL_INTERVAL_US {
            return;
        }

        self.last_ultrasonic_poll_micros = now_micros;
        scaffold.sensors.ultrasonic.enabled = true;

        match self.ultrasonic.measure_range_mm() {
            Ok(distance) => {
                let previous = scaffold.sensors.ultrasonic.last_sample;
                let telemetry = scaffold.sensors.record_range(distance, 100);
                if should_log_range_sample(previous, telemetry) {
                    defmt::info!(
                        "ultrasonic distance_mm={} quality={}",
                        telemetry.distance_mm.0,
                        telemetry.quality,
                    );
                }
            }
            Err(HcSr04Error::OutOfRange(distance)) => {
                let previous = scaffold.sensors.ultrasonic.last_sample;
                let telemetry = scaffold.sensors.record_range(distance, 0);
                if should_log_range_sample(previous, telemetry) {
                    defmt::warn!(
                        "ultrasonic out-of-range distance_mm={} quality={}",
                        telemetry.distance_mm.0,
                        telemetry.quality,
                    );
                }
            }
            Err(HcSr04Error::EchoStartTimeout | HcSr04Error::EchoEndTimeout) => {}
            Err(HcSr04Error::Trigger(_) | HcSr04Error::Echo(_)) => {
                defmt::warn!("ultrasonic gpio read/write failed");
            }
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none", feature = "sensor-ultrasonic"))]
fn should_log_range_sample(
    previous: Option<RangeTelemetry>,
    current: RangeTelemetry,
) -> bool {
    match previous {
        None => true,
        Some(previous) => {
            previous.quality != current.quality
                || previous.distance_mm.0.abs_diff(current.distance_mm.0) >= ULTRASONIC_LOG_DELTA_MM
        }
    }
}

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "driver-l298n",
    feature = "sensor-ultrasonic"
))]
impl<Driver, Ultrasonic, TriggerError, EchoError> RuntimeHardware
    for MotionDriveHardware<Driver, Ultrasonic>
where
    Driver: MotorDriver,
    Ultrasonic: UltrasonicSensor<Error = HcSr04Error<TriggerError, EchoError>>,
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
        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);
        self.last_ultrasonic_poll_micros = 0;

        if self.driver.stop_all().is_err() {
            defmt::warn!("motion drive hardware stop failed");
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn blink_stage(marker: &mut Output<'_>, pulses: usize) {
    marker.set_low();
    cortex_m::asm::delay(STAGE_BLINK_GAP_CYCLES);

    for _ in 0..pulses {
        marker.set_high();
        cortex_m::asm::delay(STAGE_BLINK_DELAY_CYCLES);
        marker.set_low();
        cortex_m::asm::delay(STAGE_BLINK_DELAY_CYCLES);
    }

    cortex_m::asm::delay(STAGE_BLINK_GAP_CYCLES);
    marker.set_high();
}

/// Transport class used by the host link.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransportClass {
    /// USB CDC serial transport.
    #[default]
    UsbCdc,
    /// UART fallback for bring-up and diagnostics.
    UartFallback,
}

/// USB transport state for the embedded target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UsbTransport {
    /// Active transport class.
    pub class: TransportClass,
}

impl UsbTransport {
    /// Construct the default USB transport.
    pub const fn new() -> Self {
        Self {
            class: TransportClass::UsbCdc,
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub async fn run_runtime(_spawner: Spawner) -> ! {
    let peripherals = embassy_rp::init(Default::default());
    let mut boot_marker = Output::new(peripherals.PIN_25, Level::Low);
    for _ in 0..4 {
        boot_marker.set_high();
        cortex_m::asm::delay(BOOT_BLINK_DELAY_CYCLES);
        boot_marker.set_low();
        cortex_m::asm::delay(BOOT_BLINK_DELAY_CYCLES);
    }
    boot_marker.set_high();

    let driver = embassy_rp::usb::Driver::new(peripherals.USB, Irqs);
    blink_stage(&mut boot_marker, 1);

    let mut config = Config::new(USB_VENDOR_ID, USB_PRODUCT_ID);
    config.manufacturer = Some("mortimmy");
    config.product = Some("mortimmy USB serial");
    config.serial_number = Some("mortimmy-rp2350");
    config.max_power = 100;
    config.max_packet_size_0 = 64;

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESCRIPTOR.init([0; 256]),
        BOS_DESCRIPTOR.init([0; 256]),
        &mut [],
        CONTROL_BUFFER.init([0; 64]),
    );
    blink_stage(&mut boot_marker, 2);

    let class = CdcAcmClass::new(&mut builder, CDC_STATE.init(CdcAcmState::new()), USB_MAX_PACKET_SIZE as u16);
    let mut device = builder.build();
    blink_stage(&mut boot_marker, 3);
    blink_stage(&mut boot_marker, 4);

    let (sender, receiver) = class.split();
    let sender = CDC_SENDER.init(sender);
    let receiver = CDC_RECEIVER.init(receiver);
    let decoder = FRAME_DECODER.init(FrameDecoder::default());
    let payload_buffer = PAYLOAD_BUFFER.init([0; MAX_PAYLOAD_LEN]);
    let frame_buffer = FRAME_BUFFER.init([0; MAX_FRAME_BODY_LEN + 1]);
    let scaffold = FIRMWARE_SCAFFOLD.init(FirmwareScaffold::default());
    #[cfg(all(feature = "driver-l298n", feature = "sensor-ultrasonic"))]
    let mut hardware = MotionDriveHardware::new(L298nDriveMotorDriver::new(
        L298nSideDriver::new(
            L298nBridge::new(
                Output::new(peripherals.PIN_3, Level::Low),
                Output::new(peripherals.PIN_4, Level::Low),
                Pwm::new_output_a(peripherals.PWM_SLICE1, peripherals.PIN_2, Default::default()),
            ),
            L298nBridge::new(
                Output::new(peripherals.PIN_5, Level::Low),
                Output::new(peripherals.PIN_6, Level::Low),
                Pwm::new_output_b(peripherals.PWM_SLICE3, peripherals.PIN_7, Default::default()),
            ),
        ),
        L298nSideDriver::new(
            L298nBridge::new(
                Output::new(peripherals.PIN_9, Level::Low),
                Output::new(peripherals.PIN_10, Level::Low),
                Pwm::new_output_a(peripherals.PWM_SLICE4, peripherals.PIN_8, Default::default()),
            ),
            L298nBridge::new(
                Output::new(peripherals.PIN_11, Level::Low),
                Output::new(peripherals.PIN_12, Level::Low),
                Pwm::new_output_b(peripherals.PWM_SLICE6, peripherals.PIN_13, Default::default()),
            ),
        ),
    ), HcSr04::new(
        Output::new(peripherals.PIN_14, Level::Low),
        Input::new(peripherals.PIN_15, embassy_rp::gpio::Pull::None),
        Delay,
        EmbassyMicrosecondClock,
    ));
    #[cfg(all(feature = "driver-l298n", not(feature = "sensor-ultrasonic")))]
    let mut hardware = MotionDriveHardware::new(L298nDriveMotorDriver::new(
        L298nSideDriver::new(
            L298nBridge::new(
                Output::new(peripherals.PIN_3, Level::Low),
                Output::new(peripherals.PIN_4, Level::Low),
                Pwm::new_output_a(peripherals.PWM_SLICE1, peripherals.PIN_2, Default::default()),
            ),
            L298nBridge::new(
                Output::new(peripherals.PIN_5, Level::Low),
                Output::new(peripherals.PIN_6, Level::Low),
                Pwm::new_output_b(peripherals.PWM_SLICE3, peripherals.PIN_7, Default::default()),
            ),
        ),
        L298nSideDriver::new(
            L298nBridge::new(
                Output::new(peripherals.PIN_9, Level::Low),
                Output::new(peripherals.PIN_10, Level::Low),
                Pwm::new_output_a(peripherals.PWM_SLICE4, peripherals.PIN_8, Default::default()),
            ),
            L298nBridge::new(
                Output::new(peripherals.PIN_11, Level::Low),
                Output::new(peripherals.PIN_12, Level::Low),
                Pwm::new_output_b(peripherals.PWM_SLICE6, peripherals.PIN_13, Default::default()),
            ),
        ),
    ));
    #[cfg(not(feature = "driver-l298n"))]
    let mut hardware = NoopRuntimeHardware;
    let usb_fut = device.run();
    let link_fut = async move {
        loop {
            receiver.wait_connection().await;
            defmt::info!("usb cdc host connected");
            let _ = usb_link_session(
                receiver,
                sender,
                decoder,
                payload_buffer,
                frame_buffer,
                scaffold,
                &mut hardware,
            )
            .await;
            defmt::warn!("usb cdc host disconnected");
        }
    };

    join(usb_fut, link_fut).await;
    unreachable!()
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn usb_link_session(
    receiver: &mut Receiver<'static, UsbDriver>,
    sender: &mut Sender<'static, UsbDriver>,
    decoder: &mut FrameDecoder,
    payload_buffer: &mut [u8; MAX_PAYLOAD_LEN],
    frame_buffer: &mut [u8; MAX_FRAME_BODY_LEN + 1],
    scaffold: &mut FirmwareScaffold,
    hardware: &mut impl RuntimeHardware,
) -> Result<(), EndpointError> {
    let mut rx_packet = [0u8; USB_MAX_PACKET_SIZE];
    *decoder = FrameDecoder::default();

    loop {
        let timeout_ms = u64::from(scaffold.control.limits.link_timeout_ms.0.max(1));
        let received = match select(
            receiver.read_packet(&mut rx_packet),
            Timer::after(EmbassyDuration::from_millis(timeout_ms)),
        )
        .await
        {
            Either::First(Ok(received)) => received,
            Either::First(Err(error)) => {
                hardware.enter_fault_state(scaffold, Some(CoreError::LinkTimedOut));
                return Err(error);
            }
            Either::Second(()) => {
                hardware.enter_fault_state(scaffold, Some(CoreError::LinkTimedOut));
                defmt::warn!("usb cdc link timeout expired after {} ms", timeout_ms as u32);
                continue;
            }
        };

        for byte in &rx_packet[..received] {
            let frame = match decoder.push(*byte) {
                Ok(Some(frame)) => frame,
                Ok(None) => continue,
                Err(_) => {
                    defmt::warn!("usb cdc frame decode error");
                    continue;
                }
            };

            let message = match decode_message(frame.payload.as_slice()) {
                Ok(message) => message,
                Err(_) => {
                    defmt::warn!("usb cdc message decode error");
                    continue;
                }
            };

            match &message {
                WireMessage::Command(command) => {
                    defmt::info!("usb cdc command={=str}", command.kind());
                }
                WireMessage::Telemetry(telemetry) => {
                    defmt::warn!("usb cdc unexpected inbound telemetry={=str}", telemetry.kind());
                }
            }

            let mut response = scaffold.apply_wire_message(message);
            if hardware.sync_with_scaffold(scaffold).is_err() {
                response = Some(WireMessage::Telemetry(Telemetry::Status(
                    scaffold.status_telemetry(),
                )));
            } else {
                refresh_response_telemetry(&mut response, scaffold);
            }

            let Some(response) = response else {
                continue;
            };

            if let WireMessage::Telemetry(telemetry) = &response {
                defmt::info!("usb cdc telemetry={=str}", telemetry.kind());
            }

            let payload = match encode_message(&response, payload_buffer) {
                Ok(payload) => payload,
                Err(_) => {
                    defmt::warn!("usb cdc response encode error");
                    continue;
                }
            };
            let encoded = match wrap_payload(payload, scaffold.link_tx.sequence, frame_buffer) {
                Ok(encoded) => encoded,
                Err(_) => {
                    defmt::warn!("usb cdc response frame error");
                    continue;
                }
            };

            if let Err(error) = write_frame(sender, encoded).await {
                hardware.enter_fault_state(scaffold, Some(CoreError::LinkTimedOut));
                return Err(error);
            }
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
fn refresh_response_telemetry(response: &mut Option<WireMessage>, scaffold: &FirmwareScaffold) {
    let Some(WireMessage::Telemetry(telemetry)) = response.as_mut() else {
        return;
    };

    *telemetry = match *telemetry {
        Telemetry::Status(_) => Telemetry::Status(scaffold.status_telemetry()),
        Telemetry::DesiredState(_) => Telemetry::DesiredState(scaffold.desired_state_telemetry()),
        other => other,
    };
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
async fn write_frame(sender: &mut Sender<'static, UsbDriver>, frame: &[u8]) -> Result<(), EndpointError> {
    let mut offset = 0;
    while offset < frame.len() {
        let end = core::cmp::min(offset + USB_MAX_PACKET_SIZE, frame.len());
        sender.write_packet(&frame[offset..end]).await?;
        offset = end;
    }

    if frame.len() % USB_MAX_PACKET_SIZE == 0 {
        sender.write_packet(&[]).await?;
    }

    Ok(())
}
