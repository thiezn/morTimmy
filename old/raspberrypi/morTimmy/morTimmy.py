#!/usr/bin/env python3

# imports
import logging
from hardware_controller import *
from time import sleep, time
import queue


class Robot:
    """ Main class for controlling our robot morTimmy

    The brain of the robot is a raspberry Pi and the low level
    electronic are handled by an Arduino. The Arduino provides
    an interface to the DC motors and various sensors
    """

    class State:
        """ Set the state of the Robot """
        running = "running"
        stopped = "stopped"
        autonomous = "autonomous"

    # Note: only variables belonging to all
    # instances of the class belong here. Others
    # should be initialised in __init__
    MIN_DISTANCE_TO_OBJECT = 10

    def __init__(self):
        """ Called when the robot class is created.

        It intializes the sensor data queue and sets up the
        logging output file

        Returns:

        Raises:
          TODO: Add proper error handling.
        """

        self.LOG_FILENAME = 'my_morTimmy.log'
        logging.basicConfig(filename=self.LOG_FILENAME,
                            level=logging.DEBUG,
                            filemode='w',
                            format='%(asctime)s %(levelname)s %(message)s')

        self.state = self.State()
        self.currentState = self.state.stopped
        self.arduino = HardwareController()
        self.runningTime = 0
        self.lastSensorReading = 0

        logging.info('initialising morTimmy the robot')
        self.sensorDataQueue = queue.Queue()
        self.initialize()

    def initialize(self):
        """ (re)initializes the robot.

        Responsible for setting up the connection to the Arduino.
        The function loops until a connection is established
        """
        self.arduino.initialize()
        while not self.arduino.isConnected:
            print ("Failed to establish connection to Arduino, retrying in 5s")
            logging.warning("Failed to establish connection to Arduino, "
                            "retrying in 5s")
            sleep(5)                # wait 5sec before trying again
            self.arduino.initialize()
        logging.info('Connected to Arduino through serial connection')
        self.runningTime = 0

    def run(self):
        """ The main robot loop """

        # Check connection to arduino, reinitialize if not
        if not self.arduino.isConnected:
            self.arduino.initialize()

        currentTime = time()

        # Turn robot randomly to the left or right when an object is near
        if self.arduino.getDistance() <= self.MIN_DISTANCE_TO_OBJECT:
            pass

        # Move robot forward if stopped for 5sec
        if self.currentState == self.state.stopped and (currentTime - self.runningTime) >= 5:
            self.arduino.sendMessage(MODULE_MOTOR, CMD_MOTOR_FORWARD, 255)
            self.runningTime = currentTime
            self.currentState = self.state.running
            print("Robot moving forward")
        # Stop robot if running for 5sec
        elif self.currentState == self.state.running and (currentTime - self.runningTime) >= 5:
            self.arduino.sendMessage(MODULE_MOTOR, CMD_MOTOR_STOP)
            self.runningTime = currentTime
            self.currentState = self.state.stopped
            print("Robot stopped")

        # Read bytes from the Arduino and add messages to the Queue if found
        self.arduino.recvMessage()

        # Process all received messages in the queue
        while not self.arduino.recvMessageQueue.empty():
            recvMessage = self.arduino.recvMessageQueue.get_nowait()

            if recvMessage is None:
                # Why does the queue always return a None object?
                break
            elif recvMessage == 'Invalid':
                logging.error('Received invalid packet, ignoring')
            elif recvMessage['module'] == chr(MODULE_DISTANCE_SENSOR):
                self.arduino.setDistance(recvMessage['data'])
            else:
                logging.warning("Message with unknown module or command received. Message details:")
                logging.warning("msgID: %d ackID: %d module: %s "
                               "commandType: %s data: %d checksum: %s" % (recvMessage['messageID'],
                                                                          recvMessage['acknowledgeID'],
                                                                          hex(recvMessage['module']),
                                                                          hex(recvMessage['commandType']),
                                                                          recvMessage['data'],
                                                                          hex(recvMessage['checksum'])))


def main():
    """ This is the main function of our script.

    It will only contain a very limited program
    logic. The main action happens in the Robot class
    """
    morTimmy = Robot()

    try:
        while(True):
            morTimmy.run()
    except KeyboardInterrupt:
        print("Thanks for running me!")

if __name__ == '__main__':
    main()
