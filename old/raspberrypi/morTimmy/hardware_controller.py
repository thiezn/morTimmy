#!/usr/bin/env python3

import serial			    # pyserial library for serial communications
import struct 	         	# Python struct library for constructing the message
import queue
from zlib import crc32      # used to calculate a message checksum
from time import sleep
import logging

# Definitions

# Frames
FRAME_FLAG = 0x0C       # Marks the start and end of a frame
FRAME_ESC = 0x1B        # Escape char for frame

# Arduino
MODULE_ARDUINO = 0x30
CMD_ARDUINO_START = 0x64
CMD_ARDUINO_START_NACK = 0x65
CMD_ARDUINO_STOP = 0x66
CMD_ARDUINO_STOP_NACK = 0x67
CMD_ARDUINO_RESTART = 0x68
CMD_ARDUINO_RESTART_NACK = 0x69

# Distance Sensor
MODULE_DISTANCE_SENSOR = 0x31
CMD_DISTANCE_SENSOR_START = 0x64
CMD_DISTANCE_SENSOR_NACK = 0x65
CMD_DISTANCE_SENSOR_STOP = 0x66
CMD_DISTANCE_SENSOR_STOP_NACK = 0x67

# Motor
MODULE_MOTOR = 0x32
CMD_MOTOR_FORWARD = 0x64
CMD_MOTOR_FORWARD_NACK = 0x65
CMD_MOTOR_BACK = 0x66
CMD_MOTOR_BACK_NACK = 0x67
CMD_MOTOR_LEFT = 0x68
CMD_MOTOR_LEFT_NACK = 0x69
CMD_MOTOR_RIGHT = 0x6A
CMD_MOTOR_RIGHT_NACK = 0x6B
CMD_MOTOR_STOP = 0x6C
CMD_MOTOR_STOP_NACK = 0x6D


class HardwareController():

    """ Serial interface into the Arduino microcontroller

    We use a protocol similar to the PPP protocol used in for
    example ADSL connections. For each message sent over the
    serial connection we will add a FRAME_FLAG to the beginning
    and end. If by change the message itself contains the FRAME_FLAG
    or other special characters we precede the byte with an
    escape flag (FRAME_ESC).

    Since data can be lost or mangled in transmission due to hardware
    errors or electrical interference we perform a CRC check on the
    raw message and append that after the message.

     Frame layout
    +------------+---------+-----+------------+
    | FRAME_FLAG | MESSAGE | CRC | FRAME_FLAG |
    +------------+---------+-----+------------+

    Our message consists of the following fields:

    messageID      (unsigned long, 4 bytes, numeric id of the message)
    acknowledgeID  (unsigned long, 4 bytes, numeric id of the messageID
                    it replied to)
    module         (unsigned char, 1 byte, Arduino module to target)
    commandType    (unsigned char, 1 byte, Type of command to send)
    data           (unsigned long, 4 bytes, the data e.g. distance )
    checksum       (unsigned long, 4 bytes, CRC32)

    The commandType depicts the action that has to take place on the
    specified module.  The data field contains data that might be returned
    like the distance from a distance sensor.

    The following modules are available currently:
        Arduino                 Controls the Arduino itself
        Motor                   Controls the robots motors
        Servo                   Controls the Servo motors
        DistanceSensor          Handles the distance sensor
        AccelerationSensor      Handles the Acceleration sensor
        CompassSensor           Handles the Compass sensor
    """

    __lastMessageID = 0        # holds the last used messageID
    isConnected = False
    __distanceSensorValues = [0, 3, 0]    # holds the last three measured vals

    def __init__(self):
        """ Initializes the HardwareController

        This sets up the recvMessageQueue which will hold
        all the received messages from the Arduino

        TODO: Implement threading/queuing for the serial
        read process
        """

        self.recvMessageQueue = queue.Queue()
        logging.getLogger()

    def setDistance(self, distance):
        """ Set the latest distance sensor value

        This function will pop the oldest value of the
        __distanceSensorValues list and amend it with the
        latest reading
        """

        self.__distanceSensorValues.pop(0)
        self.__distanceSensorValues.append(distance)
        logging.info("morTimmy: new distance "
                     "value is %s") % str(self.__distanceSensorValues)

    def getDistance(self, numOfSamples=3):
        """ get the distance measures by the distance sensor

        The distance is calculated taking the average of the last
        three measurements
        """

        return sum(self.__distanceSensorValues)/numOfSamples

    def initialize(self, serialPort='/dev/ttyACM0',
                   baudrate=9600,
                   stopbits=serial.STOPBITS_ONE,
                   bytesize=serial.EIGHTBITS,
                   timeout=0):
        """ initialize serial connection towards Arduino

        First the serial connection is opened to the arduino. Then
        we use the DTR pin to reset the arduino making sure we
        have a clean session and flushed all data from the recv
        buffer

        Args:
          serialPort (str): The port used to communicate with the Arduino
          baudrate (int): The baudrate of the serial connection
          stopbits (int): The stopbits of the serial connection
          bytesize (int): The bytesize of the serial connection
          timeout (float): The timeout for the serial read command
                           None wait forever
                           0 non blocking
                           x set timeout to x seconds (float allowed)
        """

        try:
            logging.info("Opening serial connection to arduino on"
                         "port %s with baudrate %d") % (serialPort, baudrate)
            self.serialPort = serial.Serial(serialPort, baudrate)
            logging.info("Connected to Arduino")

            '''  Reset the arduino by setting the DTR pin LOW and then
            HIGH again. This is the same as pressing the reset button
            on the Arduino itself. The flushInput() whilst the reset
            is in progress is to ensure there is no data from before
            the Arduino was reset in the serial buffer '''

            logging.info("Resetting Arduino using DTR pin")
            self.serialPort.setDTR(level=False)
            sleep(0.5)
            self.serialPort.flushInput()
            self.serialPort.setDTR()

            logging.info("TODO: implement proper handshake between Arduino "
                         "and Pi to make sure it's initalised properly")

            '''
            self.serialPort.timeout = 0.1     # Set blocking read to 5 sec
            handshake = self.serialPort.readline()
            print "Recv from Arduino: %s" % handshake
            self.serialPort.timeout = 0     # set non-blocking read
            '''
            self.isConnected = True
        except OSError:
            logging.error("Failed to connect to Arduino on "
                          "serial port %s. Is the port correct?") % serialPort
            self.isConnected = False
        except Exception:
            logging.warning("Could not connect to Arduino")
            self.isConnected = False

    def __del__(self):
        """ Close the serial connection when the class is deleted """
        try:
            self.serialPort.close()
        except:
            pass

    def __packMessage(self, module, commandType, data=0, acknowledgeID=0):
        """ Creates a message understood by the Arduino

        The checksum is calculated over the full packet with checksum field
        set to 0. The data is then repacked again with the calculated
        checksum

          Message structure
        +-----------+--------+-------------+-------+----------+
        | messageID | module | commandType |  data | checksum |
        +-----------+--------+-------------+-------+----------+

        Args:
            module:      (unsigned short, 1 byte, arduino module to target)
            commandType: (unsigned short, 1 byte, Type of command to send)
            data:        (unsigned int, 4 bytes, the data payload)

        Returns:
            Message byte string
        """

        self.__lastMessageID += 1

        checksum = 0
        rawMessage = struct.pack('<LLBBLL',
                                 self.__lastMessageID,
                                 acknowledgeID,
                                 module,
                                 commandType,
                                 data,
                                 checksum)

        # Calculate the checksum. & 0xffffffff is ensuring
        # the checksum is an unsigned long as 32bit python
        # sometimes returns a signed int
        checksum = crc32(rawMessage) & 0xffffffff

        rawMessage = struct.pack('<LLBBLL',
                                 self.__lastMessageID,
                                 acknowledgeID,
                                 module,
                                 commandType,
                                 data,
                                 checksum)

        return rawMessage

    def __unpackMessage(self, message):
        """ Unpacks a message received from the Arduino

        It unpacks the received struct into seperate variables.
        Then it repacks the message again but with a checksum of 0
        to verify the data is transmitted intact.

        A dictionary containing the received data is added to the
        recvMessageQueue if the message was valid.

        Args:
            message (struct): A message unpacked from a frame
        """
        try:
            (messageID, acknowledgeID, module, commandType,
             data, recvChecksum) = struct.unpack('<LLBBLL', message)

            # recalculate the checksum to check if we received
            # a valid message. & 0xffffffff is ensuring
            # the checksum is an unsigned long as 32bit python 2.x
            # sometimes returns a signed int

            checksum = 0
            rawMessage = struct.pack('<LLBBLL',
                                     messageID,
                                     acknowledgeID,
                                     module,
                                     commandType,
                                     data,
                                     checksum)

            calcChecksum = crc32(rawMessage) & 0xffffffff

            if recvChecksum == calcChecksum:
                self.recvMessageQueue.put({'messageID': messageID,
                                           'acknowledgeID': acknowledgeID,
                                           'module': module,
                                           'commandType': commandType,
                                           'data': data,
                                           'checksum': recvChecksum})
            else:
                self.recvMessageQueue.put("Invalid: Checksum failed")
        except Exception:
            self.recvMessageQueue.put("error putting messg to queue")

    def __packFrame(self, message):
        """ Packs the message into a frame

        Escapes any special chars and applies
        the frame marker to the beginning and end
        of the frame

        Args:
            message (struct): The message to be sent to
                              the arduino

        Returns:
            A packed frame suitable for sending to the arduino
            over the serial connection. """

        frame = b''
        frame += chr(FRAME_FLAG)

        for byte in message:
            if (byte == chr(FRAME_ESC)) or (byte == chr(FRAME_FLAG)):
                frame += chr(FRAME_ESC)
                frame += byte          # TODO, Need to XOR this value?
            else:
                frame += byte

        frame += chr(FRAME_FLAG)

        return frame

    def __unpackFrame(self, frame):
        """ Unpacks a received frame from the arduino

        Args:
            frame (string): The received frame from
                            the arduino
        Returns:
            message (struct): A received message from the
                              arduino
        """

        nextByteValid = False   # Indicates we found a FRAME_ESC char
        message = b''

        if(frame[:1] != chr(FRAME_FLAG)) or (frame[-1:] != chr(FRAME_FLAG)):
            print("Invalid frame received, frame flag not valid")
            self.recvMessageQueue.put("Invalid")
        else:
            for byte in frame:
                if nextByteValid:
                    message += byte
                    nextByteValid = False
                elif (byte == chr(FRAME_ESC)):
                        nextByteValid = True
                elif (byte != chr(FRAME_FLAG)):
                        message += byte

        return message

    def sendMessage(self, module, commandType, data=0, acknowledgeID=0):
        """ Send data onto the serial port towards the arduino.

        Used by the HardwareController class to send commands. It packs
        the message into a struct using the given arguments. The packed
        message then gets processed by __packFrame to ensure any special
        characters are escaped with FRAME_ESC and a beginning and end
        flag is added to the message.

        Args:
            module (byte):      The module to address
            commandType (byte): The command to send to the specified module
            data (int):         The data that goes with the command (if any)
        """

        if not self.isConnected:
            print("sendMessage: Not connected to Arduino")
            return None

        packedMessage = self.__packMessage(module,
                                           commandType,
                                           data,
                                           acknowledgeID)
        packedFrame = self.__packFrame(packedMessage)

        print ("morTimmy: "
               "msgID=%d "
               "ackID=%d "
               "module=%s "
               "cmd=%s "
               "data=%s ") % (self.__lastMessageID, acknowledgeID, hex(module),
                              hex(commandType), data)

        self.serialPort.write(packedFrame)

    def recvMessage(self):
        """ Receive data from the Arduino through the serial port.

        Used by the HardwareController class to receive
        messages from the Arduino. It reads single bytes from the
        serial port and starts processing when it finds the beginning
        of a message (FRAME_FLAG). each subsequent byte is read and checked
        for a FRAME_FLAG message or a FRAME_ESC escape flag for special
        characters.

        When a full message is found it is passed to the __unpackMessage
        function. This converts the received message to a dictionary and
        adds it to the recvMessageQueue.
        """

        if not self.isConnected:
            print("recvMessage: Not connected to Arduino")
            return None

        message = b''
        foundStartOfFrame = False
        foundEndOfFrame = False
        foundEscFlag = False

        while not foundEndOfFrame:
            recvByte = self.serialPort.read(1)
            if foundStartOfFrame and recvByte == chr(FRAME_FLAG) and not foundEscFlag:
                foundEndOfFrame = True
            elif recvByte == chr(FRAME_FLAG) and not foundStartOfFrame:
                # Beginning of our message
                foundStartOfFrame = True
            elif (foundStartOfFrame) and recvByte == chr(FRAME_ESC) and not foundEscFlag:
                # Found an escape flag which was not escaped itself
                foundEscFlag = True
            elif foundStartOfFrame and foundEscFlag:
                # Byte preceded by FLAG_ESC so treat as normal data
                foundEscFlag = False
                message += recvByte
            elif foundStartOfFrame and not foundEscFlag:
                # regular data part of message
                message += recvByte

        unpackedMessage = self.__unpackMessage(message)
        self.recvMessageQueue.put(unpackedMessage)


def main():
    """ This function will only be called when the library is
    run directly. Only to be used to do quick tests on the library.
    """

    try:
        arduino = HardwareController()
    except Exception as e:
        print ("Error, could not establish connection to "
               "Arduino through the serial port.\n%s") % e
#        exit()

    arduino.initialize()
    arduino.sendMessage(MODULE_MOTOR, CMD_MOTOR_FORWARD, 255)
    arduino.recvMessage()


if __name__ == '__main__':
    main()
