#![allow(dead_code)]

use core::convert::Infallible;

use embassy_rp::{
    Peripherals,
    gpio::{Level, Output},
};
use embassy_time::Instant;
use mortimmy_core::CoreError;
use mortimmy_drivers::{AudioOutput, AudioStreamConfig, PicoAudioPack, PicoAudioPackTransport};

use crate::{
    FirmwareScaffold,
    runtime::{BoardRuntime, NoopBootMarker, RuntimeHardware},
    ui::audio::AudioOutputTask,
};

const AUDIO_USB_PRODUCT: &str = "mortimmy audio controller";
const AUDIO_USB_SERIAL: &str = "mortimmy-audio-controller";

/// Bring-up audio transport that reserves the Pico Audio Pack pins while the RP2350 I2S
/// data path is still being wired in.
struct BringUpAudioTransport {
    _din: Output<'static>,
    _bck: Output<'static>,
    _lrck: Output<'static>,
    active_stream: Option<AudioStreamConfig>,
}

impl BringUpAudioTransport {
    fn new(din: Output<'static>, bck: Output<'static>, lrck: Output<'static>) -> Self {
        Self {
            _din: din,
            _bck: bck,
            _lrck: lrck,
            active_stream: None,
        }
    }
}

impl PicoAudioPackTransport for BringUpAudioTransport {
    type Error = Infallible;

    fn start_stream(&mut self, config: AudioStreamConfig) -> Result<(), Self::Error> {
        self.active_stream = Some(config);
        Ok(())
    }

    fn write_samples(&mut self, _samples: &[i16]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn stop_stream(&mut self) -> Result<(), Self::Error> {
        self.active_stream = None;
        Ok(())
    }
}

pub struct AudioControllerHardware {
    audio_output: PicoAudioPack<BringUpAudioTransport, Output<'static>>,
    last_audio_progress_ms: u64,
}

impl AudioControllerHardware {
    fn new(audio_output: PicoAudioPack<BringUpAudioTransport, Output<'static>>) -> Self {
        Self {
            audio_output,
            last_audio_progress_ms: 0,
        }
    }

    fn sync_audio(&mut self, scaffold: &mut FirmwareScaffold) {
        if scaffold.audio.queued_chunks == 0 {
            if self.audio_output.active_stream().is_some() {
                let _ = self.audio_output.stop();
            }
            self.last_audio_progress_ms = Instant::now().as_millis();
            return;
        }

        let desired_stream = scaffold.audio.stream_config();
        if self.audio_output.active_stream() != Some(desired_stream) {
            if self.audio_output.active_stream().is_some() {
                let _ = self.audio_output.stop();
            }
            let _ = self.audio_output.start(desired_stream);
            self.last_audio_progress_ms = Instant::now().as_millis();
        }

        let chunk_duration_ms = chunk_duration_ms(&scaffold.audio);
        let now_ms = Instant::now().as_millis();
        let elapsed_ms = now_ms.saturating_sub(self.last_audio_progress_ms);
        if elapsed_ms >= chunk_duration_ms {
            let completed_chunks = core::cmp::min(
                (elapsed_ms / chunk_duration_ms) as u16,
                scaffold.audio.queued_chunks,
            );
            scaffold.audio.queued_chunks = scaffold
                .audio
                .queued_chunks
                .saturating_sub(completed_chunks);
            self.last_audio_progress_ms = self
                .last_audio_progress_ms
                .saturating_add(chunk_duration_ms.saturating_mul(u64::from(completed_chunks)));
        }

        if scaffold.audio.queued_chunks == 0 && self.audio_output.active_stream().is_some() {
            let _ = self.audio_output.stop();
        }
    }
}

impl RuntimeHardware for AudioControllerHardware {
    fn sync_with_scaffold(&mut self, scaffold: &mut FirmwareScaffold) -> Result<(), ()> {
        self.sync_audio(scaffold);
        Ok(())
    }

    fn enter_fault_state(&mut self, scaffold: &mut FirmwareScaffold, error: Option<CoreError>) {
        scaffold.enter_fault_state(error);
        self.last_audio_progress_ms = 0;
        let _ = self.audio_output.stop();
    }
}

pub fn build_runtime(
    peripherals: Peripherals,
) -> BoardRuntime<NoopBootMarker, AudioControllerHardware> {
    let audio_output = PicoAudioPack::new(
        BringUpAudioTransport::new(
            Output::new(peripherals.PIN_9, Level::Low),
            Output::new(peripherals.PIN_10, Level::Low),
            Output::new(peripherals.PIN_11, Level::Low),
        ),
        Output::new(peripherals.PIN_29, Level::Low),
    );

    let hardware = AudioControllerHardware::new(audio_output);

    BoardRuntime {
        usb: peripherals.USB,
        boot_marker: NoopBootMarker,
        hardware,
        usb_product: AUDIO_USB_PRODUCT,
        usb_serial_number: AUDIO_USB_SERIAL,
    }
}

fn chunk_duration_ms(audio: &AudioOutputTask) -> u64 {
    let channels = core::cmp::max(audio.config.channels, 1) as usize;
    let frames_per_chunk = audio.config.chunk_samples / channels;
    let samples_per_second = core::cmp::max(audio.config.sample_rate_hz, 1) as u64;
    let duration_ms = (frames_per_chunk as u64).saturating_mul(1_000) / samples_per_second;
    core::cmp::max(duration_ms, 1)
}
