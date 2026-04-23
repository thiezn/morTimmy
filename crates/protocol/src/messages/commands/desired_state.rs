use core::fmt;

use mortimmy_core::Mode;
#[cfg(not(feature = "capability-drive"))]
use mortimmy_core::PwmTicks;
#[cfg(not(feature = "capability-servo"))]
use mortimmy_core::ServoTicks;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, SeqAccess, Visitor},
    ser::SerializeTuple,
};

use super::{drive::DriveCommand, servo::ServoCommand};

#[cfg(not(feature = "capability-drive"))]
const fn default_drive_command() -> DriveCommand {
    DriveCommand {
        left: PwmTicks(0),
        right: PwmTicks(0),
    }
}

#[cfg(not(feature = "capability-servo"))]
const fn default_servo_command() -> ServoCommand {
    ServoCommand {
        pan: ServoTicks(0),
        tilt: ServoTicks(0),
    }
}

/// Full continuous-control snapshot owned by the host brain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DesiredStateCommand {
    pub mode: Mode,
    #[cfg(feature = "capability-drive")]
    pub drive: DriveCommand,
    #[cfg(feature = "capability-servo")]
    pub servo: ServoCommand,
}

impl DesiredStateCommand {
    pub const fn new(mode: Mode, drive: DriveCommand, servo: ServoCommand) -> Self {
        let _ = drive;
        let _ = servo;

        Self {
            mode,
            #[cfg(feature = "capability-drive")]
            drive,
            #[cfg(feature = "capability-servo")]
            servo,
        }
    }

    pub const fn drive(&self) -> DriveCommand {
        #[cfg(feature = "capability-drive")]
        {
            self.drive
        }
        #[cfg(not(feature = "capability-drive"))]
        {
            default_drive_command()
        }
    }

    pub const fn servo(&self) -> ServoCommand {
        #[cfg(feature = "capability-servo")]
        {
            self.servo
        }
        #[cfg(not(feature = "capability-servo"))]
        {
            default_servo_command()
        }
    }
}

impl Serialize for DesiredStateCommand {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut tuple = serializer.serialize_tuple(3)?;
        tuple.serialize_element(&self.mode)?;
        tuple.serialize_element(&self.drive())?;
        tuple.serialize_element(&self.servo())?;
        tuple.end()
    }
}

impl<'de> Deserialize<'de> for DesiredStateCommand {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_tuple(3, DesiredStateCommandVisitor)
    }
}

struct DesiredStateCommandVisitor;

impl<'de> Visitor<'de> for DesiredStateCommandVisitor {
    type Value = DesiredStateCommand;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a desired state command tuple")
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

        Ok(DesiredStateCommand::new(mode, drive, servo))
    }
}

#[cfg(test)]
mod tests {
    use mortimmy_core::{Mode, PwmTicks, ServoTicks};

    use super::DesiredStateCommand;
    use crate::messages::commands::{DriveCommand, ServoCommand};

    #[cfg(not(feature = "capability-drive"))]
    use super::default_drive_command;
    #[cfg(not(feature = "capability-servo"))]
    use super::default_servo_command;

    #[test]
    fn desired_state_accessors_stay_defined_across_feature_sets() {
        let drive = DriveCommand {
            left: PwmTicks(120),
            right: PwmTicks(-80),
        };
        let servo = ServoCommand {
            pan: ServoTicks(24),
            tilt: ServoTicks(36),
        };
        let command = DesiredStateCommand::new(Mode::Teleop, drive, servo);

        #[cfg(feature = "capability-drive")]
        assert_eq!(command.drive(), drive);
        #[cfg(not(feature = "capability-drive"))]
        assert_eq!(command.drive(), default_drive_command());

        #[cfg(feature = "capability-servo")]
        assert_eq!(command.servo(), servo);
        #[cfg(not(feature = "capability-servo"))]
        assert_eq!(command.servo(), default_servo_command());
    }
}
