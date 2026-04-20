/**
 * @file morTimmy.ino
 * @brief Main program logic for morTimmy the Robot
 * @author Mathijs Mortimer
 */

// INCLUDES

#include "Arduino.h"            // required for access to the core arduino stuff like analogWrite and pinMode
#include "L298N_motor_driver.h"
#include <NewPing.h>
#include "newping_distance_sensor.h"
#include <Servo.h>
#include "raspberry_control.h"

#define RUN_TIME 30

// PIN DEFINITIONS

// DC Motor Pins
#define FRONT_LEFT_MOTOR_DIRECTION_PIN_1 1
#define FRONT_LEFT_MOTOR_DIRECTION_PIN_2 2
#define FRONT_LEFT_MOTOR_SPEED_PIN 3          // Has to be a PWM supported pin

#define REAR_LEFT_MOTOR_DIRECTION_PIN_1 4
#define REAR_LEFT_MOTOR_DIRECTION_PIN_2 5
#define REAR_LEFT_MOTOR_SPEED_PIN 6           // Has to be a PWM supported pin

#define FRONT_RIGHT_MOTOR_DIRECTION_PIN_1 7
#define FRONT_RIGHT_MOTOR_DIRECTION_PIN_2 8
#define FRONT_RIGHT_MOTOR_SPEED_PIN 9         // Has to be a PWM supported pin

#define REAR_RIGHT_MOTOR_DIRECTION_PIN_1 10
#define REAR_RIGHT_MOTOR_DIRECTION_PIN_2 11
#define REAR_RIGHT_MOTOR_SPEED_PIN 12         // Has to be a PWM supported pin

// Servo Motor Pins
#define BOTTOM_PANTILT_SERVO_PIN 3            // PWM
#define TOP_PANTILT_SERVO_PIN 4               // PWM

// Distance Sensor Pins
#define DISTANCE_SENSOR_TRIG_PIN 22
#define DISTANCE_SENSOR_ECHO_PIN 23
#define TOO_CLOSE 10                         // distance in cm to an obstacle the robot should avoid 
#define MAX_DISTANCE (TOO_CLOSE * 20)        // maximum distance in cm the sensor will measure 

// CLASS DEFINITIONS

namespace morTimmy {
    class Robot {
        public:
            /*
             * @brief Class constructor
             */
            Robot() 
                : leftMotors(FRONT_LEFT_MOTOR_DIRECTION_PIN_1, 
                             FRONT_LEFT_MOTOR_DIRECTION_PIN_2,
                             FRONT_LEFT_MOTOR_SPEED_PIN,
                             REAR_LEFT_MOTOR_DIRECTION_PIN_1,
                             REAR_LEFT_MOTOR_DIRECTION_PIN_2,
                             REAR_LEFT_MOTOR_SPEED_PIN),
                  rightMotors(FRONT_RIGHT_MOTOR_DIRECTION_PIN_1,
                              FRONT_RIGHT_MOTOR_DIRECTION_PIN_2,
                              FRONT_RIGHT_MOTOR_SPEED_PIN,
                              REAR_RIGHT_MOTOR_DIRECTION_PIN_1,
                              REAR_RIGHT_MOTOR_DIRECTION_PIN_2,
                              REAR_RIGHT_MOTOR_SPEED_PIN),
                  distanceSensor(DISTANCE_SENSOR_TRIG_PIN,
                                 DISTANCE_SENSOR_ECHO_PIN,
                                 MAX_DISTANCE),
                  raspberry()     
            {
            }


            /*
             * @brief initialize the robot
             */
            void initialize()
            {
                Serial.println("Initializing robot");
                leftMotors.setSpeed(0);
                rightMotors.setSpeed(0);
                state = stateRemote;
            }

            /*
             * @brief Update the state of the robot based on input from sensor and/or remote control.
             * Must be called repeatedly while the robot is in operation.
             */
            void run() {
              unsigned long currentTime = millis();
              //int distance = distanceAverage.add(distanceSensor.getDistance());
              int distance = distanceSensor.getDistance();

              message_t msg;              
              msg.module = MODULE_DISTANCE_SENSOR;
              msg.commandType = CMD_ARDUINO_START;
              msg.acknowledgeID = 0;
              msg.data = (unsigned long) 1;
              msg.checksum = 0;
              
              raspberry.sendMessage(msg);
              if (remoteControlled()) {
                // send the current distance to the raspberry
                //Serial.print("distance: ");
                //Serial.println(distance);
                            
                }
            }
            
            /**
              * @brief Move robot forward
              */
            void move() {
              leftMotors.setSpeed(255);
              rightMotors.setSpeed(255);
              state = stateMoving;
            }

            /**
              * @brief Stop moving Robot
              */
            void stop() {
              leftMotors.setSpeed(0);
              rightMotors.setSpeed(0);
              state = stateStopped;
            }
            
            /**
              * @brief Check if we are done running
              */
            bool doneRunning(unsigned long currentTime) {
              return (currentTime >= endTime);
            }
              
            /**
              * @brief check if there's an obstacle in sight
              */
            bool obstacleAhead(unsigned int distance) {
              return (distance <= TOO_CLOSE);
            }
            
            /**
              * @brief Turn the robot
              */
            bool turn(unsigned long currentTime) {
              if(random(2) == 0) {
                leftMotors.setSpeed(-255);
                rightMotors.setSpeed(255);
              }
              else {
                leftMotors.setSpeed(255);
                rightMotors.setSpeed(-255);
              }
              state = stateTurning;
              endStateTime = currentTime + random(500, 1000);
            }
            
            /**
              * @brief Check if we're done turning
              */
            bool doneTurning(unsigned long currentTime, unsigned int distance) {
              if (currentTime >= endStateTime) {
                return (distance > TOO_CLOSE);
              }
              return false;
            }
                    
            /**
              * @brief This gets called when state is changed to stateRemote
              */
            void remote()
            {
              leftMotors.setSpeed(0);
              rightMotors.setSpeed(0);
              state = stateRemote;
            }
            
            // Functions to check if the given state is True
            // Used for making the code more readable.
            bool stopped() { return (state == stateStopped); }
            bool moving() { return (state == stateMoving); }
            bool turning() { return (state == stateTurning); }
            bool remoteControlled() { return (state == stateRemote); }


        private:
            Motor leftMotors;                // Controls the front and rear left DC motors
            Motor rightMotors;               // Controls the front and rear right DC motors
            DistanceSensor distanceSensor;   // Holds the Distance Sensor class
            unsigned long endTime;           // end time of ??
            unsigned long endStateTime;      // current time + random value used for turning the robot
            enum state_t { stateStopped, stateMoving, stateTurning, stateRemote };    // Various robot states
            state_t state;                                  // Holds the current robot state
            unsigned long stateStartTime;                   // Holds the start time of the current state
            RaspberryController raspberry;                    // Holds the raspberry remote control class 
    };
};

morTimmy::Robot robot;

void setup() {
    Serial.begin(9600);
    robot.initialize();
}

void loop() {
    robot.run();
}
