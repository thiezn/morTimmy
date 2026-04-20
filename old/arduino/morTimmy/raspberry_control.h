/**
  * @file raspberry_controller.h
  * @brief Raspberry Controller definition for the morTimmy robot
  * @author Mathijs Mortimer
  */
  
#include "crc32.h"
 
namespace morTimmy {

  // Definitions
  // Frames
  byte FRAME_FLAG = 0x0C;       // Marks the start and end of a frame
  byte FRAME_ESC = 0x1B;        // Escape char for frame

  // Arduino
  byte MODULE_ARDUINO = 0x30;
  byte CMD_ARDUINO_START = 0x64;
  byte CMD_ARDUINO_START_NACK = 0x65;
  byte CMD_ARDUINO_STOP = 0x66;
  byte CMD_ARDUINO_STOP_NACK = 0x67;
  byte CMD_ARDUINO_RESTART = 0x68;
  byte CMD_ARDUINO_RESTART_NACK = 0x69;

  // Distance Sensor
  byte MODULE_DISTANCE_SENSOR = 0x31;
  byte CMD_DISTANCE_SENSOR_START = 0x64;
  byte CMD_DISTANCE_SENSOR_NACK = 0x65;
  byte CMD_DISTANCE_SENSOR_STOP = 0x66;
  byte CMD_DISTANCE_SENSOR_STOP_NACK = 0x67;

  // Motor
  byte MODULE_MOTOR = 0x32;
  byte CMD_MOTOR_FORWARD = 0x64;
  byte CMD_MOTOR_FORWARD_NACK = 0x65;
  byte CMD_MOTOR_BACK = 0x66;
  byte CMD_MOTOR_BACK_NACK = 0x67;
  byte CMD_MOTOR_LEFT = 0x68;
  byte CMD_MOTOR_LEFT_NACK = 0x69;
  byte CMD_MOTOR_RIGHT = 0x6A;
  byte CMD_MOTOR_RIGHT_NACK = 0x6B;
  byte CMD_MOTOR_STOP = 0x6C;
  byte CMD_MOTOR_STOP_NACK = 0x6D;

  struct message_t {
        unsigned long messageID;
        unsigned long acknowledgeID;
        byte module;        // 2 bytes, corresponds to unsigned short on Pi
        byte commandType;
        unsigned long data;        // 4 bytes, corresponds to unsigned int on Pi
        unsigned long checksum;
  };   

  class RaspberryController {
    public:

      unsigned long lastMessageID;
      /**
        * @brief Class constructor
        */
      RaspberryController() {
        lastMessageID = 0;
      }
      
      /**
        * @brief Receive and parse a message over the serial interface
        * @param msg: a message_t structure consisting of the message data
        */
      void recvMessage() {
          int numBytesAvailable = Serial.available();

          while (numBytesAvailable > 0) {
              // TODO: check if we receive start of a message and grab it until end of message is received
          }
      }

      /**
        * @brief send a message over the serial interface
        * @param msg: a message_t structure consisting of the message data
        */
      void sendMessage(message_t &msg)
      {
        // Set the messageID
        lastMessageID++;
        msg.messageID = lastMessageID;
        msg.checksum = 0;
       
        // Copy the struct in a char[]
        char byteMsg[sizeof(msg)];
        memcpy(byteMsg, &msg, sizeof(msg));

        // calculate the checksum of the message (with checksum set to 0)
        // then copy the struct again but now with the proper checksum 
        msg.checksum = crc_string(byteMsg, sizeof(byteMsg));
        memcpy(byteMsg, &msg, sizeof(msg));

        // First we iterate through the message to see if
        // there are any special chars that need escaping
        int msgSizeIncrease = 0;
        for (int i = 0; i < sizeof(byteMsg); i++) {
          if (byteMsg[i] == FRAME_FLAG || byteMsg[i] == FRAME_ESC) {
            msgSizeIncrease++;
          }
        }
        
        // If we have found special chars we create a new temp.
        // byteMsg string with the appropriate size which
        // we'll append FLAG_ESC to on the appropriate places
        if (msgSizeIncrease > 0) {

          char tmpByteMsg[sizeof(msg)+msgSizeIncrease];
          memcpy(tmpByteMsg, &byteMsg, sizeof(msg));
        
          int bytesAdded = 0;  // keeps track of how many bytes we've added to offset the memmove
          for (int i = 0; i < sizeof(byteMsg); i++) {
            if (byteMsg[i] == FRAME_FLAG || byteMsg[i] == FRAME_ESC) {
              memmove(tmpByteMsg +i + 1,
                      tmpByteMsg + i + bytesAdded,
                      sizeof(tmpByteMsg) - (i + 1));
              tmpByteMsg[i] = FRAME_ESC;
              bytesAdded++;
             }
          }
          Serial.write(FRAME_FLAG);
          Serial.write(tmpByteMsg, sizeof(tmpByteMsg));
          Serial.write(FRAME_FLAG);    
        } else {

        // Print the packet bytes to the serial port including
        // the FRAME_FLAG to the start and end of the message
        
        Serial.write(FRAME_FLAG);
        Serial.write(byteMsg, sizeof(byteMsg));
        Serial.write(FRAME_FLAG);
    
          /** Uncomment the following section if you want to 'pretty print' 
          the packets into HEX on the serial port
          
          Serial.print("Packet size: ");
          Serial.println(sizeof(byteMsg));
          
          Serial.print("0x");
          Serial.print(FRAME_FLAG, HEX);
          Serial.print(" ");
          for (int i = 0; i < sizeof(byteMsg); i++) {
            Serial.print("0x"); 
            Serial.print(byteMsg[i], HEX);
            Serial.print(" ");
          }
          Serial.print(FRAME_FLAG, HEX);
          Serial.println();
          */ 
        } 
      }      
  };
};

