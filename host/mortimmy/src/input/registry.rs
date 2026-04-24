use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::brain::BrainCommand;

use super::source::{
    CommandInputSource, ControlState, ControllerId, ControllerInfo, InputEvent, InputWarning,
};

const REGISTRY_POLL_SLICE: Duration = Duration::from_millis(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerSelection {
    Any,
    Locked(ControllerId),
}

impl Default for ControllerSelection {
    fn default() -> Self {
        Self::Any
    }
}

impl ControllerSelection {
    fn allows(&self, controller: &ControllerId) -> bool {
        match self {
            Self::Any => true,
            Self::Locked(locked) => locked == controller,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControllerLifecycleEvent {
    Connected(ControllerInfo),
    Disconnected(ControllerInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoutedInputEvent {
    Command(BrainCommand),
    Control(ControlState),
    Warning(InputWarning),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcedInputEvent {
    pub controller: ControllerId,
    pub event: RoutedInputEvent,
}

impl SourcedInputEvent {
    pub fn new(controller: ControllerId, event: RoutedInputEvent) -> Self {
        Self { controller, event }
    }
}

pub trait ControllerBackend {
    fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>>;

    fn poll_input(&mut self, timeout: Duration) -> Result<Option<SourcedInputEvent>>;

    fn suspend(&mut self) -> Result<()> {
        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        Ok(())
    }

    fn extend_active_control(
        &mut self,
        _controller: &ControllerId,
        _duration: Duration,
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct ControllerRegistry {
    selection: ControllerSelection,
    backends: Vec<Box<dyn ControllerBackend>>,
    known_controllers: BTreeMap<ControllerId, ControllerInfo>,
    pending_events: VecDeque<InputEvent>,
    active_control_state: ControlState,
    last_control_source: Option<ControllerId>,
}

impl ControllerRegistry {
    pub fn new(selection: ControllerSelection, backends: Vec<Box<dyn ControllerBackend>>) -> Self {
        Self {
            selection,
            backends,
            known_controllers: BTreeMap::new(),
            pending_events: VecDeque::new(),
            active_control_state: ControlState::default(),
            last_control_source: None,
        }
    }

    #[allow(dead_code)]
    pub fn active_controllers(&self) -> impl Iterator<Item = &ControllerInfo> {
        self.known_controllers.values()
    }

    fn queue_lifecycle_event(&mut self, event: ControllerLifecycleEvent) {
        match event {
            ControllerLifecycleEvent::Connected(info) => {
                self.known_controllers.insert(info.id.clone(), info.clone());
                self.pending_events
                    .push_back(InputEvent::ControllerConnected(info));
            }
            ControllerLifecycleEvent::Disconnected(info) => {
                self.known_controllers.remove(&info.id);
                let should_clear_control = self.last_control_source.as_ref() == Some(&info.id)
                    && self.active_control_state != ControlState::default();

                if self.last_control_source.as_ref() == Some(&info.id) {
                    self.last_control_source = None;
                }

                self.pending_events
                    .push_back(InputEvent::ControllerDisconnected(info));

                if should_clear_control {
                    self.active_control_state = ControlState::default();
                    self.pending_events
                        .push_back(InputEvent::Control(self.active_control_state));
                }
            }
        }
    }

    fn refresh_controllers(&mut self) -> Result<()> {
        let mut lifecycle_events = Vec::new();

        for backend in &mut self.backends {
            lifecycle_events.extend(backend.refresh_controllers()?);
        }

        for event in lifecycle_events {
            self.queue_lifecycle_event(event);
        }

        Ok(())
    }

    fn queue_routed_event(&mut self, event: SourcedInputEvent) {
        if !self.selection.allows(&event.controller) {
            return;
        }

        match event.event {
            RoutedInputEvent::Command(command) => {
                self.pending_events.push_back(InputEvent::Command(command));
            }
            RoutedInputEvent::Control(control_state) => {
                self.active_control_state = control_state;
                self.last_control_source = if control_state.drive.is_some() {
                    Some(event.controller)
                } else {
                    None
                };
                self.pending_events
                    .push_back(InputEvent::Control(control_state));
            }
            RoutedInputEvent::Warning(warning) => {
                self.pending_events.push_back(InputEvent::Warning(warning));
            }
        }
    }

    fn poll_backends_once(&mut self, timeout: Duration) -> Result<bool> {
        let mut timeout_budget = timeout;

        for backend in &mut self.backends {
            if let Some(event) = backend.poll_input(timeout_budget)? {
                self.queue_routed_event(event);
                return Ok(true);
            }

            timeout_budget = Duration::ZERO;
        }

        Ok(false)
    }
}

impl CommandInputSource for ControllerRegistry {
    fn next_event(&mut self) -> Result<InputEvent> {
        loop {
            if let Some(event) = self.poll_event(Duration::from_millis(250))? {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self, timeout: Duration) -> Result<Option<InputEvent>> {
        self.refresh_controllers()?;
        if let Some(event) = self.pending_events.pop_front() {
            return Ok(Some(event));
        }

        if self.backends.is_empty() {
            return Ok(None);
        }

        if timeout.is_zero() {
            while self.poll_backends_once(Duration::ZERO)? {
                if let Some(event) = self.pending_events.pop_front() {
                    return Ok(Some(event));
                }
            }

            return Ok(self.pending_events.pop_front());
        }

        let started_at = Instant::now();
        while started_at.elapsed() < timeout {
            let remaining = timeout.saturating_sub(started_at.elapsed());
            let had_backend_event = self.poll_backends_once(remaining.min(REGISTRY_POLL_SLICE))?;
            self.refresh_controllers()?;

            if let Some(event) = self.pending_events.pop_front() {
                return Ok(Some(event));
            }

            if had_backend_event {
                continue;
            }
        }

        Ok(None)
    }

    fn suspend(&mut self) -> Result<()> {
        self.pending_events.clear();
        self.known_controllers.clear();
        self.active_control_state = ControlState::default();
        self.last_control_source = None;

        for backend in &mut self.backends {
            backend.suspend()?;
        }

        Ok(())
    }

    fn resume(&mut self) -> Result<()> {
        self.pending_events.clear();
        self.known_controllers.clear();
        self.active_control_state = ControlState::default();
        self.last_control_source = None;

        for backend in &mut self.backends {
            backend.resume()?;
        }

        self.refresh_controllers()
    }

    fn extend_active_control(&mut self, duration: Duration) -> Result<()> {
        let Some(controller) = self.last_control_source.clone() else {
            return Ok(());
        };

        for backend in &mut self.backends {
            backend.extend_active_control(&controller, duration)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::rc::Rc;
    use std::time::Duration;

    use anyhow::Result;

    use crate::input::{
        CommandInputSource, ControllerId, ControllerInfo, ControllerKind, DriveIntent,
    };

    use super::{
        ControlState, ControllerBackend, ControllerLifecycleEvent, ControllerRegistry,
        ControllerSelection, InputEvent, RoutedInputEvent, SourcedInputEvent,
    };

    #[derive(Debug, Default)]
    struct MockBackendState {
        refreshes: VecDeque<Vec<ControllerLifecycleEvent>>,
        events: VecDeque<SourcedInputEvent>,
        extended: Vec<(ControllerId, Duration)>,
    }

    struct MockBackend {
        state: Rc<RefCell<MockBackendState>>,
    }

    impl MockBackend {
        fn new(state: Rc<RefCell<MockBackendState>>) -> Self {
            Self { state }
        }
    }

    impl ControllerBackend for MockBackend {
        fn refresh_controllers(&mut self) -> Result<Vec<ControllerLifecycleEvent>> {
            Ok(self
                .state
                .borrow_mut()
                .refreshes
                .pop_front()
                .unwrap_or_default())
        }

        fn poll_input(&mut self, _timeout: Duration) -> Result<Option<SourcedInputEvent>> {
            Ok(self.state.borrow_mut().events.pop_front())
        }

        fn extend_active_control(
            &mut self,
            controller: &ControllerId,
            duration: Duration,
        ) -> Result<()> {
            self.state
                .borrow_mut()
                .extended
                .push((controller.clone(), duration));
            Ok(())
        }
    }

    fn controller(kind: ControllerKind, instance: &str, display_name: &str) -> ControllerInfo {
        ControllerInfo::new(ControllerId::new(kind, instance), display_name)
    }

    fn drive_state(forward: i16, turn: i16) -> ControlState {
        ControlState {
            drive: Some(DriveIntent {
                forward,
                turn,
                speed: 300,
            }),
        }
    }

    #[test]
    fn disconnecting_active_controller_clears_control_state() {
        let keyboard = controller(ControllerKind::Keyboard, "local", "Local Keyboard");
        let state = Rc::new(RefCell::new(MockBackendState {
            refreshes: VecDeque::from([
                vec![ControllerLifecycleEvent::Connected(keyboard.clone())],
                vec![],
                vec![ControllerLifecycleEvent::Disconnected(keyboard.clone())],
            ]),
            events: VecDeque::from([SourcedInputEvent::new(
                keyboard.id.clone(),
                RoutedInputEvent::Control(drive_state(DriveIntent::AXIS_MAX, 0)),
            )]),
            extended: Vec::new(),
        }));

        let mut registry = ControllerRegistry::new(
            ControllerSelection::Any,
            vec![Box::new(MockBackend::new(state))],
        );

        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerConnected(keyboard.clone()))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::Control(drive_state(DriveIntent::AXIS_MAX, 0)))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerDisconnected(keyboard))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::Control(ControlState::default()))
        );
    }

    #[test]
    fn locked_selection_filters_other_controllers() {
        let keyboard = controller(ControllerKind::Keyboard, "local", "Local Keyboard");
        let websocket = controller(ControllerKind::Websocket, "client-a", "Websocket Client");
        let state = Rc::new(RefCell::new(MockBackendState {
            refreshes: VecDeque::from([vec![
                ControllerLifecycleEvent::Connected(keyboard.clone()),
                ControllerLifecycleEvent::Connected(websocket.clone()),
            ]]),
            events: VecDeque::from([
                SourcedInputEvent::new(
                    keyboard.id.clone(),
                    RoutedInputEvent::Control(drive_state(DriveIntent::AXIS_MAX, 0)),
                ),
                SourcedInputEvent::new(
                    websocket.id.clone(),
                    RoutedInputEvent::Control(drive_state(0, DriveIntent::AXIS_MAX)),
                ),
            ]),
            extended: Vec::new(),
        }));

        let locked = websocket.id.clone();
        let mut registry = ControllerRegistry::new(
            ControllerSelection::Locked(locked.clone()),
            vec![Box::new(MockBackend::new(state.clone()))],
        );

        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerConnected(keyboard))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerConnected(websocket.clone()))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::Control(drive_state(0, DriveIntent::AXIS_MAX)))
        );

        registry
            .extend_active_control(Duration::from_millis(250))
            .unwrap();

        assert_eq!(
            state.borrow().extended,
            vec![(locked, Duration::from_millis(250))]
        );
    }

    #[test]
    fn any_selection_lets_latest_control_win() {
        let keyboard = controller(ControllerKind::Keyboard, "local", "Local Keyboard");
        let gamepad = controller(ControllerKind::GamepadUsb, "usb-0", "USB Gamepad");
        let state = Rc::new(RefCell::new(MockBackendState {
            refreshes: VecDeque::from([vec![
                ControllerLifecycleEvent::Connected(keyboard.clone()),
                ControllerLifecycleEvent::Connected(gamepad.clone()),
            ]]),
            events: VecDeque::from([
                SourcedInputEvent::new(
                    keyboard.id.clone(),
                    RoutedInputEvent::Control(drive_state(DriveIntent::AXIS_MAX, 0)),
                ),
                SourcedInputEvent::new(
                    gamepad.id.clone(),
                    RoutedInputEvent::Control(drive_state(0, -DriveIntent::AXIS_MAX)),
                ),
            ]),
            extended: Vec::new(),
        }));

        let mut registry = ControllerRegistry::new(
            ControllerSelection::Any,
            vec![Box::new(MockBackend::new(state.clone()))],
        );

        assert!(matches!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerConnected(_))
        ));
        assert!(matches!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::ControllerConnected(_))
        ));
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::Control(drive_state(DriveIntent::AXIS_MAX, 0)))
        );
        assert_eq!(
            registry.poll_event(Duration::ZERO).unwrap(),
            Some(InputEvent::Control(drive_state(0, -DriveIntent::AXIS_MAX)))
        );

        registry
            .extend_active_control(Duration::from_millis(100))
            .unwrap();

        assert_eq!(
            state.borrow().extended,
            vec![(gamepad.id, Duration::from_millis(100))]
        );
    }
}
