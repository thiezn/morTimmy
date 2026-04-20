/**
 * @file distance_sensor.h
 * @brief Distance sensor driver definition for the morTimmy robot.
 * @athor Mathijs Mortimer
 */
 
namespace morTimmy {
  class DistanceSensorDriver {
    public:
      /**
        * @brief Class constructor.
        * @param distance The maximum distance in cm that needs to be tracked.
        */
      DistanceSensorDriver(unsigned int distance) : maxDistance(distance) {}
      
      /**
        * @brief Return the distance to the nearest obstacle in cm
        * @return the distance to the closest object in cm
        *   or maxDistance if no object was detected
        */
      virtual unsigned int getDistance() = 0;
    
    protected:
      unsigned int maxDistance;
  };
};

