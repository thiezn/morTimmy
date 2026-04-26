use core::fmt;

#[cfg(not(feature = "capability-drive"))]
use mortimmy_core::PwmTicks;
#[cfg(not(feature = "capability-servo"))]
use mortimmy_core::ServoTicks;
use mortimmy_core::{CoreError, Mode};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
    ser::SerializeTuple,
};

use super::{RangeTelemetry, drive::MotorStateTelemetry, servo::ServoStateTelemetry};

#[cfg(not(feature = "capability-drive"))]
const fn default_drive_telemetry() -> MotorStateTelemetry {
    MotorStateTelemetry {
        left_pwm: PwmTicks(0),
        right_pwm: PwmTicks(0),
        current_limit_hit: false,
    }
}

#[cfg(not(feature = "capability-servo"))]
const fn default_servo_telemetry() -> ServoStateTelemetry {
    ServoStateTelemetry {
        pan: ServoTicks(0),
        tilt: ServoTicks(0),
    }
}

/// Immediate acknowledgement of the latest applied desired-control state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesiredStateTelemetry {
    pub mode: Mode,
    #[cfg(feature = "capability-drive")]
    pub drive: MotorStateTelemetry,
    #[cfg(feature = "capability-servo")]
    pub servo: ServoStateTelemetry,
    pub error: Option<CoreError>,
    pub range: Option<RangeTelemetry>,
}

impl DesiredStateTelemetry {
    pub const fn new(
        mode: Mode,
        drive: MotorStateTelemetry,
        servo: ServoStateTelemetry,
        error: Option<CoreError>,
        range: Option<RangeTelemetry>,
    ) -> Self {
        let _ = drive;
        let _ = servo;

        Self {
            mode,
            #[cfg(feature = "capability-drive")]
            drive,
            #[cfg(feature = "capability-servo")]
            servo,
            error,
            range,
        }
    }

    pub const fn drive(&self) -> MotorStateTelemetry {
        #[cfg(feature = "capability-drive")]
        {
            self.drive
        }
        #[cfg(not(feature = "capability-drive"))]
        {
            default_drive_telemetry()
        }
    }

    pub const fn servo(&self) -> ServoStateTelemetry {
        #[cfg(feature = "capability-servo")]
        {
            self.servo
        }
        #[cfg(not(feature = "capability-servo"))]
        {
            default_servo_telemetry()
        }
    }
}

impl Serialize for DesiredStateTelemetry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tuple = serializer.serialize_tuple(5)?;
        tuple.serialize_element(&self.mode)?;
        tuple.serialize_element(&self.drive())?;
        tuple.serialize_element(&self.servo())?;
        tuple.serialize_element(&self.error)?;
        tuple.serialize_element(&self.range)?;
        tuple.end()
    }
}

impl<'de> Deserialize<'de> for DesiredStateTelemetry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(5, DesiredStateTelemetryVisitor)
    }
}

struct DesiredStateTelemetryVisitor;

impl<'de> Visitor<'de> for DesiredStateTelemetryVisitor {
    type Value = DesiredStateTelemetry;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a desired state telemetry tuple")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mode = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let drive = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        let servo = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
        let error = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(3, &self))?;
        let range = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(4, &self))?;

        Ok(DesiredStateTelemetry::new(mode, drive, servo, error, range))
    }
}

#[cfg(test)]
mod tests {
    use mortimmy_core::{Mode, PwmTicks, ServoTicks};

    use super::DesiredStateTelemetry;
    use crate::messages::telemetry::{MotorStateTelemetry, ServoStateTelemetry};

    #[cfg(not(feature = "capability-drive"))]
    use super::default_drive_telemetry;
    #[cfg(not(feature = "capability-servo"))]
    use super::default_servo_telemetry;

    #[test]
    fn desired_state_telemetry_accessors_stay_defined_across_feature_sets() {
        let drive = MotorStateTelemetry {
            left_pwm: PwmTicks(120),
            right_pwm: PwmTicks(-80),
            current_limit_hit: false,
        };
        let servo = ServoStateTelemetry {
            pan: ServoTicks(24),
            tilt: ServoTicks(36),
        };
        let telemetry = DesiredStateTelemetry::new(Mode::Teleop, drive, servo, None, None);

        #[cfg(feature = "capability-drive")]
        assert_eq!(telemetry.drive(), drive);
        #[cfg(not(feature = "capability-drive"))]
        assert_eq!(telemetry.drive(), default_drive_telemetry());

        #[cfg(feature = "capability-servo")]
        assert_eq!(telemetry.servo(), servo);
        #[cfg(not(feature = "capability-servo"))]
        assert_eq!(telemetry.servo(), default_servo_telemetry());

        assert_eq!(telemetry.range, None);
    }
}
