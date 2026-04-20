/**
 * @file L298N_motor_driver.h
 * @brief Motor device driver for the L298N motor driver controller
 * @author Mathijs Mortimer
 */
#include "motor_driver.h"

namespace morTimmy {
    class Motor : public MotorDriver {
        public:
            /*
             * @brief Class constructor.
             * @param motorXDir1, motorXDir2: controls the motor direction
             * @param motorXspeed: constrols the motor speed, has to be a PWM pin
             */
            
            Motor(int motorADir1, int motorADir2, int motorASpeed, int motorBDir1, int motorBDir2, int motorBSpeed) 
                  : _motorADir1(motorADir1), _motorADir2(motorADir2), _motorASpeed(motorASpeed), 
                    _motorBDir1(motorADir1), _motorBDir2(motorADir2), _motorBSpeed(motorASpeed), 
                    MotorDriver(), currentSpeed(0) {
                // define the L298N Dual H-Bridge Motor Controller Pins
                pinMode(_motorADir1, OUTPUT);
                pinMode(_motorADir2, OUTPUT);
                pinMode(_motorASpeed, OUTPUT);
                pinMode(_motorBDir1, OUTPUT);
                pinMode(_motorBDir2, OUTPUT);
                pinMode(_motorBSpeed, OUTPUT);
            }

            void setSpeed(int speed) {
                currentSpeed = speed;
                if (speed >= 0) {
                    // Motor A
                    analogWrite(_motorASpeed, speed);     // Set speed through PWM
                    digitalWrite(_motorADir1, LOW);      // Move motor forward/stop
                    digitalWrite(_motorADir2, HIGH);     // Move motor forward/stop
                    // Motor B
                    analogWrite(_motorBSpeed, speed);     // Set speed through PWM
                    digitalWrite(_motorBDir1, LOW);      // Move motor forward/stop
                    digitalWrite(_motorBDir2, HIGH);     // Move motor forward/stop
                }
                else {
                    // Motor A
                    analogWrite(_motorASpeed, -speed);       // Set negative speed through PWM
                    digitalWrite(_motorADir1, HIGH);         // Move motor backwards 
                    digitalWrite(_motorADir2, LOW);          // Move motor backwards
                    // Motor B
                    analogWrite(_motorASpeed, -speed);    // Set negative speed through PWM
                    digitalWrite(_motorADir1, HIGH);         // Move motor backwards
                    digitalWrite(_motorADir2, LOW);          // Move motor backwards
                }
            }

            int getSpeed() const {
                return currentSpeed;
            }

        private:
            int currentSpeed;
            uint8_t _motorADir1;
            uint8_t _motorADir2;
            uint8_t _motorASpeed;
            uint8_t _motorBDir1;
            uint8_t _motorBDir2;
            uint8_t _motorBSpeed;
    };
};

