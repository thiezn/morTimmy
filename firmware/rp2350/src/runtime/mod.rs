#![allow(dead_code)]

#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "board-audio-controller"
))]
pub mod audio;
#[cfg(all(
    target_arch = "arm",
    target_os = "none",
    feature = "board-motion-controller"
))]
pub mod motion;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use embassy_rp::{gpio::Output, peripherals};
#[cfg(all(target_arch = "arm", target_os = "none"))]
use mortimmy_core::CoreError;

#[cfg(all(target_arch = "arm", target_os = "none"))]
use crate::FirmwareScaffold;

/// Shared USB identity and concrete hardware for one board-targeted firmware image.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub struct BoardRuntime<Boot, Hardware> {
    pub usb: peripherals::USB,
    pub boot_marker: Boot,
    pub hardware: Hardware,
    pub usb_product: &'static str,
    pub usb_serial_number: &'static str,
}

/// Live board-owned hardware hook invoked from the USB session loop.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub trait RuntimeHardware {
    fn sync_with_scaffold(&mut self, scaffold: &mut FirmwareScaffold) -> Result<(), ()>;

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>);
}

/// Minimal boot-stage LED abstraction because not every board exposes a direct GPIO LED.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub trait BootMarker {
    fn set_high(&mut self);

    fn set_low(&mut self);
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl BootMarker for Output<'_> {
    fn set_high(&mut self) {
        Output::set_high(self);
    }

    fn set_low(&mut self) {
        Output::set_low(self);
    }
}

/// No-op boot marker for boards where the status LED is not a plain GPIO.
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[derive(Debug, Default)]
pub struct NoopBootMarker;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl BootMarker for NoopBootMarker {
    fn set_high(&mut self) {}

    fn set_low(&mut self) {}
}

/// Fallback runtime hardware used when no live peripheral bridge is active.
#[cfg(all(target_arch = "arm", target_os = "none"))]
#[derive(Debug, Default)]
pub struct NoopRuntimeHardware;

#[cfg(all(target_arch = "arm", target_os = "none"))]
impl RuntimeHardware for NoopRuntimeHardware {
    fn sync_with_scaffold(&mut self, _scaffold: &mut FirmwareScaffold) -> Result<(), ()> {
        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);
    }
}
