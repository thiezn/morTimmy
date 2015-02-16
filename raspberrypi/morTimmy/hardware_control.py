#!/usr/bin/env python


class HardwareController():
    """ High level interface towards the hardware layer

    It will be able to send and receive data from the microcontroller.
    """

    def sendCommand(self, command, data=''):
        """ Send a command to the hardware controller.

        The command and data size will be predetermined and combined
        into a single string. This string will be sent to the arduino
        for parsing.

        Args:
          command (str): A command can be less than commandSize and will
            add trailing whitespaces to meet the required commandSize.
          data (str): The data is not mandatory. If no data is provided it will
            be padded with whitespaces to meet the required dataSize.
        """
        pass

    def recvCommand(self):
        """ Retrieve data from the hardware controller

        Returns:
          self.__recv (str): Returns a command received from the hardware
            controller
        """
        return


def main():
    """ This function will only be called when the library is run directly
    Only to be used to do quick tests on the library.
    """

    print "Hello World, this is the generic hardware driver library"

    hwControl = HardwareController()
    hwControl.sendCommand('FD', '255')

if __name__ == '__main__':
    main()
