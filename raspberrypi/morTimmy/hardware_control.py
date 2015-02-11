#!/usr/bin/env python


class HardwareController():
    """ This class is an abstraction layer to allow communication
        to the low level hardware layer. It will be able to send
        and receive data from the microcontroller.
    """

    commandSize = 4      # Size of the recv and send command in Bytes
    dataSize = 12        # Size of the recv and send data in Bytes

    def sendCommand(self, command, data=''):
        """ This function will send a command to the specified module.
            The command and data size will be predetermined and combined
            into a single string. This string will be sent to the arduino
            for parsing.

            A command can be less than commandSize and will
            add trailing whitespaces to meet the required commandSize.

            The data is not mandatory. If no data is provided it will
            be padded with whitespaces to meet the required dataSize.
        """

        if len(command) > self.commandSize:
            print ("Error: command %s is invalid. Size should be %d "
                   "or smaller" % (command, self.commandSize))
            return
        if len(data) > self.dataSize:
            print ("Error: data %s is invalid. Size should be %d "
                   "or smaller" % (data, self.dataSize))
            return

        sendString = ''.join([command.ljust(self.commandSize, ' '),
                              data.ljust(self.dataSize, ' ')])
        self.__send(sendString)

    def getCommand(self, module):
        """ This function will retrieve data from the specified module """
        return self.__recv()

    def __send(self, data):
        print "Sent: %s" % data

    def __recv(self):
        print "Recv: <create child class of HardwareController>"
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