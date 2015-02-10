#!/usr/bin/env python

from hardware_control import HardwareController
import serial


class ArduinoSerialController(HardwareController):
    """ This class is an abstraction layer to allow communication
        to the low level hardware layer. It will be able to send
        and receive data from the microcontroller.
    """

    def __init__(self, serialPort='/dev/ttyS0',
                 baudrate=115200,
                 stopbits=serial.STOPBITS_ONE,
                 bytesize=serial.EIGHTBITS):
        """ The initialisation for the ArduinoSerialController class
            It requires the ID of the communication channel to the
            microcontroller.
        """
        self.channel = serial.Serial(serialPort, baudrate)

    def __del__(self):
        """ Close the serial connection when the class is deleted """
        self.channel.close()

    def __send(self, data):
        """ This function sends data onto the serial connection
            towards the arduino. It's used by the generic
            HardwareController class to send commands towards
            the Arduino.
        """
        self.channel.write(data)

    def __recv(self):
        """ This function receives data from the Arduino through
            the serial connection. It's used by the generic
            HardwareController class to send commands towards the
            Arduino.
        """
        return self.channel.read(self.commandSize+self.dataSize)


def main():
    """ This function will only be called when the library is run directly
        Only to be used to do quick tests on the library.
    """

    try:
        hwControl = ArduinoSerialController()
    except Exception as e:
        print ("Error, could not establish connection to "
               "Arduino through the serial port.\n%s") % e
        exit()
    hwControl.sendCommand('FWD', '255')
    hwControl.sendCommand('STOP')
    hwControl.sendCommand('FAULTY COMMAND')


if __name__ == '__main__':
    main()
