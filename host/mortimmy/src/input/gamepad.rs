use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::{Duration, Instant};

use anyhow::Result;
use gilrs::{Axis, Button, EventType, Gamepad, GamepadId, Gilrs, PowerInfo};
use hidapi::{BusType, HidApi};
use mortimmy_core::Mode;

use crate::brain::BrainCommand;

use super::{
    ControlState, ControllerBackend, ControllerId, ControllerInfo, ControllerKind,
    ControllerLifecycleEvent, DriveIntent, RoutedInputEvent, SourcedInputEvent,
};

const GAMEPAD_DRIVE_SPEED: u16 = 300;
const GAMEPAD_AXIS_DEADZONE: f32 = 0.20;
const GAMEPAD_PUMP_COALESCE: Duration = Duration::from_millis(5);

type SharedGamepadRuntime = Rc<RefCell<GamepadRuntime>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum GamepadDriveStyle {
    #[default]
    Arcade,
    Tank,
}

impl GamepadDriveStyle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Arcade => "arcade",
            Self::Tank => "tank",
        }
    }

    const fn toggled(self) -> Self {
        match self {
            Self::Arcade => Self::Tank,
            Self::Tank => Self::Arcade,
        }
    }
}

#[derive(Clone, Debug)]
struct HidDeviceSnapshot {
    vendor_id: u16,
    product_id: u16,
    product_name: Option<String>,
    bus_type: BusType,
}

#[derive(Clone, Debug)]
struct GamepadDescriptor {
    id: GamepadId,
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    name: String,
    os_name: String,
    power_info: PowerInfo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct GamepadDriveState {
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_y: f32,
    dpad_x: f32,
    dpad_y: f32,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
}

impl GamepadDriveState {
    fn set_axis(&mut self, axis: Axis, value: f32) -> bool {
        let slot = match axis {
            Axis::LeftStickX => &mut self.left_stick_x,
            Axis::LeftStickY => &mut self.left_stick_y,
            Axis::RightStickY => &mut self.right_stick_y,
            Axis::DPadX => &mut self.dpad_x,
            Axis::DPadY => &mut self.dpad_y,
            _ => return false,
        };

        if (*slot - value).abs() < f32::EPSILON {
            return false;
        }

        *slot = value;
        true
    }

    fn set_button(&mut self, button: Button, pressed: bool) -> bool {
        let slot = match button {
            Button::DPadDown => &mut self.dpad_down,
            Button::DPadLeft => &mut self.dpad_left,
            Button::DPadRight => &mut self.dpad_right,
            _ => return false,
        };

        if *slot == pressed {
            return false;
        }

        *slot = pressed;
        true
    }

    fn to_control_state(self, style: GamepadDriveStyle) -> ControlState {
        let (forward, turn) = match style {
            GamepadDriveStyle::Arcade => {
                let dpad_x = match (self.dpad_right, self.dpad_left) {
                    (true, false) => 1.0,
                    (false, true) => -1.0,
                    _ => self.dpad_x,
                };
                // Reserve D-pad up for drive-style toggling while still allowing
                // backward and left/right digital fallback in arcade mode.
                let dpad_y = if self.dpad_down {
                    -1.0
                } else {
                    (-self.dpad_y).min(0.0)
                };

                let turn = normalized_axis_value(preferred_axis(self.left_stick_x, dpad_x));
                let forward = normalized_axis_value(preferred_axis(-self.left_stick_y, dpad_y));
                (forward, turn)
            }
            GamepadDriveStyle::Tank => {
                let left_track = normalized_axis_value(-self.left_stick_y);
                let right_track = normalized_axis_value(-self.right_stick_y);
                let forward = ((i32::from(left_track) + i32::from(right_track)) / 2) as i16;
                let turn = ((i32::from(right_track) - i32::from(left_track)) / 2) as i16;
                (forward, turn)
            }
        };

        let drive = if forward == 0 && turn == 0 {
            None
        } else {
            Some(DriveIntent {
                forward,
                turn,
                speed: GAMEPAD_DRIVE_SPEED,
            })
        };

        ControlState { drive }
    }
}

#[derive(Clone, Debug)]
struct TrackedGamepad {
    kind: ControllerKind,
    info: ControllerInfo,
    drive_style: GamepadDriveStyle,
    drive_state: GamepadDriveState,
    control_state: ControlState,
}

impl TrackedGamepad {
    fn toggle_drive_style(&mut self) -> (GamepadDriveStyle, bool) {
        self.drive_style = self.drive_style.toggled();
        self.drive_state = GamepadDriveState::default();
        let was_active = self.control_state != ControlState::default();
        self.control_state = ControlState::default();
        (self.drive_style, was_active)
    }
}

trait GamepadTransportClassifier {
    fn refresh(&mut self);
    fn classify(&self, descriptor: &GamepadDescriptor) -> ControllerKind;
}

struct HidGamepadTransportClassifier {
    api: Option<HidApi>,
    devices: Vec<HidDeviceSnapshot>,
}

impl HidGamepadTransportClassifier {
    fn new() -> Self {
        Self {
            api: HidApi::new().ok(),
            devices: Vec::new(),
        }
    }
}

impl GamepadTransportClassifier for HidGamepadTransportClassifier {
    fn refresh(&mut self) {
        let Some(api) = &mut self.api else {
            self.devices.clear();
            return;
        };

        if api.refresh_devices().is_err() {
            self.devices.clear();
            return;
        }

        self.devices = api
            .device_list()
            .map(|device| HidDeviceSnapshot {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                product_name: device.product_string().map(str::to_owned),
                bus_type: device.bus_type(),
            })
            .collect();
    }

    fn classify(&self, descriptor: &GamepadDescriptor) -> ControllerKind {
        classify_transport(descriptor, &self.devices)
    }
}

struct GamepadRuntime {
    gilrs: Gilrs,
    classifier: Box<dyn GamepadTransportClassifier>,
    tracked: HashMap<GamepadId, TrackedGamepad>,
    usb_lifecycle_events: VecDeque<ControllerLifecycleEvent>,
    bluetooth_lifecycle_events: VecDeque<ControllerLifecycleEvent>,
    usb_input_events: VecDeque<SourcedInputEvent>,
    bluetooth_input_events: VecDeque<SourcedInputEvent>,
    last_pumped_at: Option<Instant>,
}

impl GamepadRuntime {
    fn new() -> Result<Self> {
        Ok(Self {
            gilrs: Gilrs::new().map_err(|error| {
                anyhow::anyhow!("failed to initialize gilrs gamepad runtime: {error}")
            })?,
            classifier: Box::new(HidGamepadTransportClassifier::new()),
            tracked: HashMap::new(),
            usb_lifecycle_events: VecDeque::new(),
            bluetooth_lifecycle_events: VecDeque::new(),
            usb_input_events: VecDeque::new(),
            bluetooth_input_events: VecDeque::new(),
            last_pumped_at: None,
        })
    }

    fn clear_runtime_state(&mut self) {
        self.tracked.clear();
        self.usb_lifecycle_events.clear();
        self.bluetooth_lifecycle_events.clear();
        self.usb_input_events.clear();
        self.bluetooth_input_events.clear();
        self.last_pumped_at = None;
    }

    fn pump(&mut self) {
        if self
            .last_pumped_at
            .is_some_and(|last_pumped_at| last_pumped_at.elapsed() < GAMEPAD_PUMP_COALESCE)
        {
            return;
        }

        self.classifier.refresh();
        self.sync_connected_gamepads();

        while let Some(event) = self.gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    self.ensure_tracked(event.id);
                }
                EventType::Disconnected => {
                    self.remove_tracked(event.id);
                }
                EventType::AxisChanged(axis, value, _) => {
                    self.ensure_tracked(event.id);
                    self.update_axis(event.id, axis, value);
                }
                EventType::ButtonPressed(button, _) => {
                    self.ensure_tracked(event.id);
                    if button == Button::DPadUp {
                        self.toggle_drive_style(event.id);
                        continue;
                    }
                    self.update_button(event.id, button, true);
                    if let Some(command) = command_for_button(button) {
                        self.push_input_event(event.id, RoutedInputEvent::Command(command));
                    }
                }
                EventType::ButtonReleased(button, _) => {
                    if button == Button::DPadUp {
                        continue;
                    }
                    self.update_button(event.id, button, false);
                }
                EventType::ButtonRepeated(_, _)
                | EventType::ButtonChanged(_, _, _)
                | EventType::Dropped
                | EventType::ForceFeedbackEffectCompleted => {}
                _ => {}
            }
        }

        self.sync_connected_gamepads();
        self.last_pumped_at = Some(Instant::now());
    }

    fn sync_connected_gamepads(&mut self) {
        let snapshots: Vec<_> = self
            .gilrs
            .gamepads()
            .filter_map(|(id, gamepad)| {
                if !gamepad.is_connected() {
                    return None;
                }

                Some(snapshot_gamepad(id, gamepad))
            })
            .collect();

        let connected_ids: Vec<_> = snapshots.iter().map(|snapshot| snapshot.id).collect();

        for snapshot in snapshots {
            self.upsert_snapshot(snapshot);
        }

        let disconnected: Vec<_> = self
            .tracked
            .keys()
            .copied()
            .filter(|tracked_id| !connected_ids.contains(tracked_id))
            .collect();

        for gamepad_id in disconnected {
            self.remove_tracked(gamepad_id);
        }
    }

    fn ensure_tracked(&mut self, gamepad_id: GamepadId) {
        if self.tracked.contains_key(&gamepad_id) {
            return;
        }

        let snapshot = snapshot_gamepad(gamepad_id, self.gilrs.gamepad(gamepad_id));
        self.upsert_snapshot(snapshot);
    }

    fn upsert_snapshot(&mut self, snapshot: GamepadDescriptor) {
        let kind = self.classifier.classify(&snapshot);
        let info = controller_info(kind, &snapshot);

        match self.tracked.get_mut(&snapshot.id) {
            Some(tracked) if tracked.kind == kind && tracked.info == info => {}
            Some(tracked) => {
                let previous_kind = tracked.kind;
                let previous_info = tracked.info.clone();

                tracked.kind = kind;
                tracked.info = info.clone();

                self.lifecycle_queue_mut(previous_kind)
                    .push_back(ControllerLifecycleEvent::Disconnected(previous_info));
                self.lifecycle_queue_mut(kind)
                    .push_back(ControllerLifecycleEvent::Connected(info));
            }
            None => {
                self.lifecycle_queue_mut(kind)
                    .push_back(ControllerLifecycleEvent::Connected(info.clone()));

                self.tracked.insert(
                    snapshot.id,
                    TrackedGamepad {
                        kind,
                        info,
                        drive_style: GamepadDriveStyle::default(),
                        drive_state: GamepadDriveState::default(),
                        control_state: ControlState::default(),
                    },
                );
            }
        }
    }

    fn remove_tracked(&mut self, gamepad_id: GamepadId) {
        let Some(tracked) = self.tracked.remove(&gamepad_id) else {
            return;
        };

        self.lifecycle_queue_mut(tracked.kind)
            .push_back(ControllerLifecycleEvent::Disconnected(tracked.info));
    }

    fn update_axis(&mut self, gamepad_id: GamepadId, axis: Axis, value: f32) {
        let Some(tracked) = self.tracked.get_mut(&gamepad_id) else {
            return;
        };

        if !tracked.drive_state.set_axis(axis, value) {
            return;
        }

        self.sync_control_event(gamepad_id);
    }

    fn update_button(&mut self, gamepad_id: GamepadId, button: Button, pressed: bool) {
        let Some(tracked) = self.tracked.get_mut(&gamepad_id) else {
            return;
        };

        if !tracked.drive_state.set_button(button, pressed) {
            return;
        }

        self.sync_control_event(gamepad_id);
    }

    fn toggle_drive_style(&mut self, gamepad_id: GamepadId) {
        let Some((kind, controller_id, display_name, drive_style, was_active)) = ({
            let Some(tracked) = self.tracked.get_mut(&gamepad_id) else {
                return;
            };

            let (drive_style, was_active) = tracked.toggle_drive_style();

            Some((
                tracked.kind,
                tracked.info.id.clone(),
                tracked.info.display_name.clone(),
                drive_style,
                was_active,
            ))
        }) else {
            return;
        };

        if was_active {
            self.input_queue_mut(kind).push_back(SourcedInputEvent::new(
                controller_id.clone(),
                RoutedInputEvent::Control(ControlState::default()),
            ));
        }

        self.input_queue_mut(kind).push_back(SourcedInputEvent::new(
            controller_id,
            RoutedInputEvent::Warning(super::InputWarning::Status(Cow::Owned(format!(
                "{} drive style switched to {}",
                display_name,
                drive_style.as_str(),
            )))),
        ));
    }

    fn sync_control_event(&mut self, gamepad_id: GamepadId) {
        let Some(tracked) = self.tracked.get_mut(&gamepad_id) else {
            return;
        };

        let control_state = tracked.drive_state.to_control_state(tracked.drive_style);
        if control_state == tracked.control_state {
            return;
        }

        tracked.control_state = control_state;
        let controller = tracked.info.id.clone();
        let kind = tracked.kind;

        self.input_queue_mut(kind).push_back(SourcedInputEvent::new(
            controller,
            RoutedInputEvent::Control(control_state),
        ));
    }

    fn push_input_event(&mut self, gamepad_id: GamepadId, event: RoutedInputEvent) {
        let Some((kind, controller_id)) = self
            .tracked
            .get(&gamepad_id)
            .map(|tracked| (tracked.kind, tracked.info.id.clone()))
        else {
            return;
        };

        self.input_queue_mut(kind)
            .push_back(SourcedInputEvent::new(controller_id, event));
    }

    fn drain_lifecycle_events(&mut self, filter: ControllerKind) -> Vec<ControllerLifecycleEvent> {
        self.lifecycle_queue_mut(filter).drain(..).collect()
    }

    fn pop_input_event(&mut self, filter: ControllerKind) -> Option<SourcedInputEvent> {
        self.input_queue_mut(filter).pop_front()
    }

    fn lifecycle_queue_mut(
        &mut self,
        filter: ControllerKind,
    ) -> &mut VecDeque<ControllerLifecycleEvent> {
        match filter {
            ControllerKind::GamepadUsb => &mut self.usb_lifecycle_events,
            ControllerKind::GamepadBluetooth => &mut self.bluetooth_lifecycle_events,
            _ => unreachable!("gamepad runtime only supports USB/Bluetooth filters"),
        }
    }

    fn input_queue_mut(&mut self, filter: ControllerKind) -> &mut VecDeque<SourcedInputEvent> {
        match filter {
            ControllerKind::GamepadUsb => &mut self.usb_input_events,
            ControllerKind::GamepadBluetooth => &mut self.bluetooth_input_events,
            _ => unreachable!("gamepad runtime only supports USB/Bluetooth filters"),
        }
    }
}

#[derive(Clone)]
struct GamepadBackend {
    runtime: SharedGamepadRuntime,
    filter: ControllerKind,
    publish_instructions: bool,
}

impl GamepadBackend {
    fn new(
        runtime: SharedGamepadRuntime,
        filter: ControllerKind,
        publish_instructions: bool,
    ) -> Self {
        Self {
            runtime,
            filter,
            publish_instructions,
        }
    }

    fn instructions_text() -> &'static str {
        "Gamepad commands:\n  left stick           Arcade drive, or left tread in tank mode\n  right stick Y        Right tread in tank mode\n  d-pad up             Toggle arcade/tank drive mode\n  d-pad left/right/down Arcade digital fallback\n  south / A / cross    Stop the robot\n  start / menu         Switch to teleop mode\n  mode / guide         Switch to autonomous mode\n  select / back        Switch to fault mode\n"
    }
}

impl ControllerBackend for GamepadBackend {
    fn instructions(&self) -> Option<Cow<'static, str>> {
        if self.publish_instructions {
            Some(Cow::Borrowed(Self::instructions_text()))
        } else {
            None
        }
    }

    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
        let mut runtime = self.runtime.borrow_mut();
        runtime.pump();
        Ok(runtime.drain_lifecycle_events(self.filter))
    }

    fn poll_input(&mut self, _timeout: Duration) -> Result<Option<SourcedInputEvent>> {
        let mut runtime = self.runtime.borrow_mut();
        runtime.pump();
        Ok(runtime.pop_input_event(self.filter))
    }

    fn suspend(&mut self) -> Result<()> {
        self.runtime.borrow_mut().clear_runtime_state();
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.runtime.borrow_mut().clear_runtime_state();
        Ok(())
    }
}

pub struct UsbGamepadInput {
    backend: GamepadBackend,
}

impl UsbGamepadInput {
    fn new(runtime: SharedGamepadRuntime) -> Self {
        Self {
            backend: GamepadBackend::new(runtime, ControllerKind::GamepadUsb, true),
        }
    }
}

impl ControllerBackend for UsbGamepadInput {
    fn instructions(&self) -> Option<Cow<'static, str>> {
        self.backend.instructions()
    }

    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
        self.backend.refresh_controllers()
    }

    fn poll_input(&mut self, timeout: Duration) -> Result<Option<SourcedInputEvent>> {
        self.backend.poll_input(timeout)
    }

    fn suspend(&mut self) -> Result<()> {
        self.backend.suspend()
    }

    fn resume(&mut self) -> Result<()> {
        self.backend.resume()
    }
}

pub struct BluetoothGamepadInput {
    backend: GamepadBackend,
}

impl BluetoothGamepadInput {
    fn new(runtime: SharedGamepadRuntime) -> Self {
        Self {
            backend: GamepadBackend::new(runtime, ControllerKind::GamepadBluetooth, false),
        }
    }
}

impl ControllerBackend for BluetoothGamepadInput {
    fn instructions(&self) -> Option<Cow<'static, str>> {
        self.backend.instructions()
    }

    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
        self.backend.refresh_controllers()
    }

    fn poll_input(&mut self, timeout: Duration) -> Result<Option<SourcedInputEvent>> {
        self.backend.poll_input(timeout)
    }

    fn suspend(&mut self) -> Result<()> {
        self.backend.suspend()
    }

    fn resume(&mut self) -> Result<()> {
        self.backend.resume()
    }
}

pub fn create_gamepad_inputs() -> Result<(UsbGamepadInput, BluetoothGamepadInput)> {
    let runtime = Rc::new(RefCell::new(GamepadRuntime::new()?));

    Ok((
        UsbGamepadInput::new(runtime.clone()),
        BluetoothGamepadInput::new(runtime),
    ))
}

fn snapshot_gamepad(id: GamepadId, gamepad: Gamepad<'_>) -> GamepadDescriptor {
    GamepadDescriptor {
        id,
        vendor_id: gamepad.vendor_id(),
        product_id: gamepad.product_id(),
        name: gamepad.name().to_string(),
        os_name: gamepad.os_name().to_string(),
        power_info: gamepad.power_info(),
    }
}

fn classify_transport(
    descriptor: &GamepadDescriptor,
    devices: &[HidDeviceSnapshot],
) -> ControllerKind {
    let mut best_usb = 0;
    let mut best_bluetooth = 0;

    for device in devices {
        let score = match_score(descriptor, device);
        if score == 0 {
            continue;
        }

        match device.bus_type {
            BusType::Usb => best_usb = best_usb.max(score),
            BusType::Bluetooth => best_bluetooth = best_bluetooth.max(score),
            _ => {}
        }
    }

    match best_usb.cmp(&best_bluetooth) {
        std::cmp::Ordering::Greater if best_usb > 0 => ControllerKind::GamepadUsb,
        std::cmp::Ordering::Less if best_bluetooth > 0 => ControllerKind::GamepadBluetooth,
        _ => classify_transport_from_power_info(descriptor.power_info),
    }
}

fn match_score(descriptor: &GamepadDescriptor, device: &HidDeviceSnapshot) -> u32 {
    let mut score = 0;

    match descriptor.vendor_id {
        Some(vendor_id) if vendor_id == device.vendor_id => score += 100,
        Some(_) => return 0,
        None => {}
    }

    match descriptor.product_id {
        Some(product_id) if product_id == device.product_id => score += 100,
        Some(_) => return 0,
        None => {}
    }

    if names_match(
        &descriptor.name,
        &descriptor.os_name,
        device.product_name.as_deref(),
    ) {
        score += 40;
    }

    score
}

fn names_match(name: &str, os_name: &str, product_name: Option<&str>) -> bool {
    let Some(product_name) = product_name else {
        return false;
    };

    let product_name = normalize_label(product_name);
    let mapping_name = normalize_label(name);
    let os_name = normalize_label(os_name);

    (!product_name.is_empty()
        && (mapping_name.contains(&product_name) || product_name.contains(&mapping_name)))
        || (!product_name.is_empty()
            && (os_name.contains(&product_name) || product_name.contains(&os_name)))
}

fn normalize_label(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn classify_transport_from_power_info(power_info: PowerInfo) -> ControllerKind {
    match power_info {
        PowerInfo::Wired => ControllerKind::GamepadUsb,
        PowerInfo::Discharging(_) | PowerInfo::Charging(_) | PowerInfo::Charged => {
            ControllerKind::GamepadBluetooth
        }
        PowerInfo::Unknown => ControllerKind::GamepadUsb,
    }
}

fn controller_info(kind: ControllerKind, descriptor: &GamepadDescriptor) -> ControllerInfo {
    let (instance_prefix, bus_label) = match kind {
        ControllerKind::GamepadUsb => ("usb", "USB"),
        ControllerKind::GamepadBluetooth => ("bluetooth", "Bluetooth"),
        _ => unreachable!("gamepad controller info only supports USB/Bluetooth kinds"),
    };

    ControllerInfo::new(
        ControllerId::new(
            kind,
            format!("{instance_prefix}-{}", usize::from(descriptor.id)),
        ),
        format!("{bus_label} gamepad: {}", descriptor.name),
    )
}

fn command_for_button(button: Button) -> Option<BrainCommand> {
    match button {
        Button::South => Some(BrainCommand::Stop),
        Button::Start => Some(BrainCommand::SetMode(Mode::Teleop)),
        Button::Mode => Some(BrainCommand::SetMode(Mode::Autonomous)),
        Button::Select => Some(BrainCommand::SetMode(Mode::Fault)),
        _ => None,
    }
}

fn preferred_axis(primary: f32, fallback: f32) -> f32 {
    if primary.abs() >= GAMEPAD_AXIS_DEADZONE {
        primary
    } else {
        fallback
    }
}

fn normalized_axis_value(value: f32) -> i16 {
    let value = if value.abs() < GAMEPAD_AXIS_DEADZONE {
        0.0
    } else {
        value.clamp(-1.0, 1.0)
    };

    (value * f32::from(DriveIntent::AXIS_MAX)).round() as i16
}

#[cfg(test)]
mod tests {
    use gilrs::PowerInfo;
    use hidapi::BusType;

    use crate::input::{ControlState, ControllerInfo, ControllerKind, DriveIntent};

    use super::{
        GamepadDescriptor, GamepadDriveState, GamepadDriveStyle, HidDeviceSnapshot, TrackedGamepad,
        classify_transport, classify_transport_from_power_info,
    };

    #[test]
    fn hid_bus_type_wins_when_match_is_unambiguous() {
        let descriptor = GamepadDescriptor {
            id: unsafe { std::mem::zeroed() },
            vendor_id: Some(0x054c),
            product_id: Some(0x0ce6),
            name: "DualSense Wireless Controller".to_string(),
            os_name: "DualSense Wireless Controller".to_string(),
            power_info: PowerInfo::Discharging(80),
        };

        let devices = vec![HidDeviceSnapshot {
            vendor_id: 0x054c,
            product_id: 0x0ce6,
            product_name: Some("DualSense Wireless Controller".to_string()),
            bus_type: BusType::Usb,
        }];

        assert_eq!(
            classify_transport(&descriptor, &devices),
            ControllerKind::GamepadUsb
        );
    }

    #[test]
    fn power_info_breaks_usb_bluetooth_ties() {
        let descriptor = GamepadDescriptor {
            id: unsafe { std::mem::zeroed() },
            vendor_id: Some(0x045e),
            product_id: Some(0x0b13),
            name: "Xbox Wireless Controller".to_string(),
            os_name: "Xbox Wireless Controller".to_string(),
            power_info: PowerInfo::Charging(55),
        };

        let devices = vec![
            HidDeviceSnapshot {
                vendor_id: 0x045e,
                product_id: 0x0b13,
                product_name: Some("Xbox Wireless Controller".to_string()),
                bus_type: BusType::Usb,
            },
            HidDeviceSnapshot {
                vendor_id: 0x045e,
                product_id: 0x0b13,
                product_name: Some("Xbox Wireless Controller".to_string()),
                bus_type: BusType::Bluetooth,
            },
        ];

        assert_eq!(
            classify_transport(&descriptor, &devices),
            ControllerKind::GamepadBluetooth
        );
    }

    #[test]
    fn power_info_fallback_maps_wired_to_usb() {
        assert_eq!(
            classify_transport_from_power_info(PowerInfo::Wired),
            ControllerKind::GamepadUsb
        );
    }

    #[test]
    fn power_info_fallback_maps_battery_to_bluetooth() {
        assert_eq!(
            classify_transport_from_power_info(PowerInfo::Discharging(42)),
            ControllerKind::GamepadBluetooth
        );
    }

    #[test]
    fn left_stick_maps_to_drive_control() {
        let mut state = GamepadDriveState::default();
        state.left_stick_y = -1.0;
        state.left_stick_x = 0.25;

        assert_eq!(
            state.to_control_state(GamepadDriveStyle::Arcade),
            ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 250,
                    speed: 300,
                }),
            }
        );
    }

    #[test]
    fn dpad_buttons_supply_arcade_digital_fallback() {
        let state = GamepadDriveState {
            dpad_down: true,
            dpad_right: true,
            ..GamepadDriveState::default()
        };

        assert_eq!(
            state.to_control_state(GamepadDriveStyle::Arcade),
            ControlState {
                drive: Some(DriveIntent {
                    forward: -DriveIntent::AXIS_MAX,
                    turn: DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            }
        );
    }

    #[test]
    fn arcade_mode_ignores_dpad_up_axis_reserved_for_toggle() {
        let state = GamepadDriveState {
            dpad_y: -1.0,
            ..GamepadDriveState::default()
        };

        assert_eq!(
            state.to_control_state(GamepadDriveStyle::Arcade),
            ControlState { drive: None }
        );
    }

    #[test]
    fn tank_mode_maps_sticks_into_forward_motion() {
        let state = GamepadDriveState {
            left_stick_y: -1.0,
            right_stick_y: -1.0,
            ..GamepadDriveState::default()
        };

        assert_eq!(
            state.to_control_state(GamepadDriveStyle::Tank),
            ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            }
        );
    }

    #[test]
    fn tank_mode_opposite_sticks_pivot_in_place() {
        let state = GamepadDriveState {
            left_stick_y: -1.0,
            right_stick_y: 1.0,
            ..GamepadDriveState::default()
        };

        assert_eq!(
            state.to_control_state(GamepadDriveStyle::Tank),
            ControlState {
                drive: Some(DriveIntent {
                    forward: 0,
                    turn: -DriveIntent::AXIS_MAX,
                    speed: 300,
                }),
            }
        );
    }

    #[test]
    fn toggling_gamepad_drive_style_resets_active_control() {
        let controller_info = ControllerInfo::new(
            crate::input::ControllerId::new(ControllerKind::GamepadUsb, "usb-0"),
            "USB gamepad: Example",
        );

        let mut tracked = TrackedGamepad {
            kind: ControllerKind::GamepadUsb,
            info: controller_info.clone(),
            drive_style: GamepadDriveStyle::Arcade,
            drive_state: GamepadDriveState {
                left_stick_y: -1.0,
                ..GamepadDriveState::default()
            },
            control_state: ControlState {
                drive: Some(DriveIntent {
                    forward: DriveIntent::AXIS_MAX,
                    turn: 0,
                    speed: 300,
                }),
            },
        };

        let (drive_style, was_active) = tracked.toggle_drive_style();

        assert!(was_active);
        assert_eq!(drive_style, GamepadDriveStyle::Tank);
        assert_eq!(tracked.kind, ControllerKind::GamepadUsb);
        assert_eq!(tracked.info, controller_info);
        assert_eq!(tracked.drive_state, GamepadDriveState::default());
        assert_eq!(tracked.control_state, ControlState::default());
    }
}
