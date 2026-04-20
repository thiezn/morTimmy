/**
  * @file remote_control.h
  * @brief remote control driver definition for the morTimmy robot
  * @author Mathijs Mortimer
  */
  
#include "remote_control.h"
 
namespace morTimmy {
  class RemoteControl : public RemoteControlDriver {
    public:    
      /**
        * @brief Class constructor
        */
      RemoteControl() : RemoteControlDriver(), lastKey(command_t::keyNone) {}
      
      
      /**
        * @brief Return the next remote command, if available
        * @param cmd A reference to a command_t struct where the
                     information will be stored
        * @ return true if a remote command is available, false if not
        */
        
      virtual bool getRemoteCommand(command_t& cmd) {
        cmd.stop();
        cmd.key = command_t::keyNone;
        
        if (Serial.available() <= 0)
          return false; // no commands available
        char ch = Serial.read();
        switch(ch) {
         case 'W': // up
           cmd.goForward(255);
           break;
         case 'S': // down
           cmd.goBack(255);
         case 'A': // Left
           cmd.goLeft(255);
         case 'D': // Right
           cmd.goRight(255);
         case '1': // Function key #1
           cmd.key = command_t::keyF1;
           break;
         case '2': // Function key #2
           cmd.key = command_t::keyF2;
           break;
         case '3': // Function key #3
           cmd.key = command_t::keyF3;
           break;
         case '4': // Function key #4
           cmd.key = command_t::keyF4;
           break;
         default:
           break;
        }
        if (cmd.key != command_t::keyNone && cmd.key == lastKey) {
          // Repeated key, ignore it
          return false;
        }
        lastKey = cmd.key;
        return true;
      }
      
      private:
        command_t::key_t lastKey;
           
  };
};

