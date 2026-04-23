use std::time::Duration;

use mortimmy_core::ServoTicks;
use mortimmy_protocol::messages::commands::ServoCommand;
use tokio::time::Instant;

use crate::input::DriveIntent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutonomousTarget {
    pub drive: Option<DriveIntent>,
    pub servo: ServoCommand,
}

impl AutonomousTarget {
    pub const fn hold_position() -> Self {
        Self {
            drive: None,
            servo: ServoCommand {
                pan: ServoTicks(0),
                tilt: ServoTicks(0),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutonomyCondition {
    After(Duration),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutonomyStep {
    pub target: AutonomousTarget,
    pub until: AutonomyCondition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutonomyPlan {
    pub name: &'static str,
    pub steps: &'static [AutonomyStep],
}

const SERVO_SCAN_STEPS: [AutonomyStep; 4] = [
    AutonomyStep {
        target: AutonomousTarget {
            drive: None,
            servo: ServoCommand {
                pan: ServoTicks(0),
                tilt: ServoTicks(0),
            },
        },
        until: AutonomyCondition::After(Duration::from_millis(500)),
    },
    AutonomyStep {
        target: AutonomousTarget {
            drive: None,
            servo: ServoCommand {
                pan: ServoTicks(48),
                tilt: ServoTicks(0),
            },
        },
        until: AutonomyCondition::After(Duration::from_millis(500)),
    },
    AutonomyStep {
        target: AutonomousTarget {
            drive: None,
            servo: ServoCommand {
                pan: ServoTicks(0),
                tilt: ServoTicks(48),
            },
        },
        until: AutonomyCondition::After(Duration::from_millis(500)),
    },
    AutonomyStep {
        target: AutonomousTarget::hold_position(),
        until: AutonomyCondition::After(Duration::from_secs(60)),
    },
];

pub const SERVO_SCAN_PLAN: AutonomyPlan = AutonomyPlan {
    name: "servo-scan",
    steps: &SERVO_SCAN_STEPS,
};

#[derive(Debug)]
pub struct AutonomyRunner {
    plan: &'static AutonomyPlan,
    step_index: usize,
    step_started_at: Option<Instant>,
}

impl AutonomyRunner {
    pub const fn servo_scan() -> Self {
        Self::new(&SERVO_SCAN_PLAN)
    }

    pub const fn new(plan: &'static AutonomyPlan) -> Self {
        Self {
            plan,
            step_index: 0,
            step_started_at: None,
        }
    }

    pub fn reset(&mut self) {
        self.step_index = 0;
        self.step_started_at = None;
    }

    pub const fn plan_name(&self) -> &'static str {
        self.plan.name
    }

    pub fn target_at(&mut self, now: Instant) -> AutonomousTarget {
        if self.step_started_at.is_none() {
            self.step_started_at = Some(now);
            return self.plan.steps[self.step_index].target;
        }

        while self.step_index + 1 < self.plan.steps.len() {
            let started_at = self.step_started_at.expect("autonomy step start time must exist");
            let step = self.plan.steps[self.step_index];
            let AutonomyCondition::After(duration) = step.until;

            if now.duration_since(started_at) < duration {
                break;
            }

            self.step_index += 1;
            self.step_started_at = Some(now);
        }

        self.plan.steps[self.step_index].target
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::{AutonomyCondition, AutonomyPlan, AutonomyRunner, AutonomyStep, AutonomousTarget};

    #[test]
    fn servo_scan_plan_advances_after_elapsed_durations() {
        let mut runner = AutonomyRunner::servo_scan();
        let start = Instant::now();

        let first = runner.target_at(start);
        let second = runner.target_at(start + Duration::from_millis(600));
        let third = runner.target_at(start + Duration::from_millis(1_200));

        assert_eq!(first.servo.pan.0, 0);
        assert_eq!(second.servo.pan.0, 48);
        assert_eq!(third.servo.tilt.0, 48);
    }

    #[test]
    fn hold_position_plan_stays_stationary() {
        const HOLD_POSITION_STEPS: [AutonomyStep; 1] = [AutonomyStep {
            target: AutonomousTarget::hold_position(),
            until: AutonomyCondition::After(Duration::from_secs(60)),
        }];
        const HOLD_POSITION_PLAN: AutonomyPlan = AutonomyPlan {
            name: "hold-position",
            steps: &HOLD_POSITION_STEPS,
        };

        let mut runner = AutonomyRunner::new(&HOLD_POSITION_PLAN);
        let start = Instant::now();
        let current = runner.target_at(start + Duration::from_secs(5));

        assert!(current.drive.is_none());
        assert_eq!(current.servo.pan.0, 0);
        assert_eq!(current.servo.tilt.0, 0);
    }
}
