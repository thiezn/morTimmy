#!/usr/bin/env python3


class ControllerDriver:
    """ Generic class for remote controlling morTimmy the Robot

    It will be used to control the Arduino microcontroller to
    perform various low level functions like driving and reading
    sensor data.

    This class will also be used to control the Raspberry Pi
    using external remote controls like a game controller or
    bluetooth phone application
    """


class ControllerCmd:
    """ Command definition for controller drivers

    This class defines the various commands our robot morTimmy
    can respond to. It's used by both the arduino/raspberry pi
    interface and remote control devices interfacing with the
    Raspberry Pi.
    """

    leftMotorSpeed = 0      # Controls the speed of the left side motors
    rightMotorSpeed = 0     # Controls the speed of the right side motors

    def goForward(self, speed):
        self.leftMotorSpeed = speed
        self.rightMotorSpeed = speed

    def goBack(self, speed):
        self.leftMotorSpeed = -speed
        self.rightMotorSpeed = -speed

    def goLeft(self, speed):
        self.leftMotorSpeed = -speed
        self.rightMotorSpeed = speed

    def goRight(self, speed):
        self.leftMotorSpeed = speed
        self.rightMotorSpeed = -speed

    def stop(self):
        self.leftMotorSpeed = 0
        self.rightMotorSpeed = 0

    def joystick(self, x, y):
        """ Controlling the robot using a joystick

        Args:
            x (int): x-axis of the joystick, controls the amount of
                     steering
            y (int): y-axis if the joystick, controls the
                     forward/back speed
        """
        self.leftMotorsSpeed = x - y
        self.rightMotorsSpeed = x + y

        # Make sure the remote control x and y values
        # do not exceed the maximum speed
        if (self.leftMotorsSpeed < -255):
            self.leftMotorsSpeed = -255
        elif (self.leftMotorsSpeed > 255):
            self.leftMotorsSpeed = 255
        if (self.rightMotorsSpeed < -255):
            self.rightMotorsSpeed = -255
        elif (self.rightMotorsSpeed > 255):
            self.rightMotorsSpeed = 255


def main():
    pass

if __name__ == '__main__':
    main()
