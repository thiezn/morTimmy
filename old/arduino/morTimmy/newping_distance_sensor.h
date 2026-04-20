/**
  * @file newping_distance_sensor.h
  * @brief distance sensr driver for distance sensors supported bt the NewPing library
  * @author Mathijs Mortimer
  */

#include "distance_sensor.h"

namespace morTimmy {
  class DistanceSensor : public DistanceSensorDriver {
    public:
      DistanceSensor(int triggerPin, int echoPin, int maxDistance)
        : DistanceSensorDriver(maxDistance),
          sensor(triggerPin, echoPin, maxDistance)
      {
      }
      
      virtual unsigned int getDistance() {
        int distance = sensor.ping_cm();
        if (distance <= 0)
          return maxDistance;
        return distance;
      }
      
    private:
      NewPing sensor;
  };
};

