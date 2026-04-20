/**
 * @file motor_driver.h
 * @brief Motor device driver definition for the morTimmy robot
 * @author Mathijs Mortimer
 */

namespace morTimmy {
    class MotorDriver {
        public:
            /**
             * @brief Change the speed and direction of the motor
             * @param speed The new speed of the motor.
             *  valid values are between -255 and 255.
             *  Use positive values to run the motor forward,
             *  negative values to run it backward and zero
             *  to stop the motor.
             */
            virtual void setSpeed(int speed) = 0;

            /**
             * @brief Return the current speed of the motor.
             * @return The current speed of the motor with range -255 to 255.
             */
            virtual int getSpeed() const = 0;
    };
};

