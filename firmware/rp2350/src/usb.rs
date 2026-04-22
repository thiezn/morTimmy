#![allow(dead_code)]

#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_futures::join::join;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_executor::Spawner;
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_futures::select::{Either, select};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_rp::gpio::{Level, Output};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_rp::{bind_interrupts, peripherals, usb};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_time::{Duration as EmbassyDuration, Timer};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_usb::{Builder, Config, class::cdc_acm::{CdcAcmClass, Receiver, Sender, State as CdcAcmState}, driver::EndpointError};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use mortimmy_protocol::{FrameDecoder, MAX_FRAME_BODY_LEN, MAX_PAYLOAD_LEN, decode_message, encode_message, wrap_payload};
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
    let usb_fut = device.run();
    let link_fut = async move {
        loop {
            receiver.wait_connection().await;
            defmt::info!("usb cdc host connected");
            let _ = usb_link_session(receiver, sender, decoder, payload_buffer, frame_buffer, scaffold).await;
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
) -> Result<(), EndpointError> {
    let mut rx_packet = [0u8; USB_MAX_PACKET_SIZE];
    *decoder = FrameDecoder::default();
    *scaffold = FirmwareScaffold::default();

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
                scaffold.restore_default_state(Some(mortimmy_core::CoreError::LinkTimedOut));
                return Err(error);
            }
            Either::Second(()) => {
                scaffold.restore_default_state(Some(mortimmy_core::CoreError::LinkTimedOut));
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

            let Some(response) = scaffold.apply_wire_message(message) else {
                continue;
            };

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

            write_frame(sender, encoded).await?;
        }
    }
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
