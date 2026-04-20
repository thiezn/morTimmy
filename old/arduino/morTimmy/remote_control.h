/**
  * @file remote_control.h
  * @brief remote control driver definition for the morTimmy robot
  * @author Mathijs Mortimer
  */
 
namespace morTimmy {
  class RemoteControlDriver {
    public:
      /**
        * @brief abstract representation of a remote command.
        */
      struct command_t {
        enum key_t { keyNone, keyF1, keyF2, keyF3, keyF4 };
        int left;  // Left side speed, between -255 and 255.
        int right; // Right side speed, between -255 and 255.
        key_t key; // function key
        
        command_t() : left(0), right(0), key(keyNone) {}
        
        // conversion functions
        void goForward(int speed) {
        left = right = speed;
        }
        void goBack(int speed) {
          left = right = -speed;
        }
        void goLeft(int speed) {
          left = -speed;
          right = speed;
        }
        void goRight(int speed) {
          left = speed;
          right = -speed;
        }
        void stop() {
          left = right = 0;
        }
        void joystick(int x, int y) {
          left = x - y;
          right = x + y;

          // Correct values to min/max speed if neccesary
          if (left < -255) {
            left = -255;
          }
          else if (left > 255) {
            left = 255;
          }
          if (right < -255) {
            right = -255;
          }
          else if (right > 255) {
            right = 255;
          }
        }
      };
      
      /**
        * @brief Class constructor
        */
      RemoteControlDriver() {}
      
      /**
        * @brief Return the next remote command, if available
        * @param cmd A reference to a command_t struct where the
                     information will be stored
        * @ return true if a remote command is available, false if not
        */
        
      virtual bool getRemoteCommand(command_t& cmd) = 0;
  };
};

