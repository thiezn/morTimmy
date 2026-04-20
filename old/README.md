# Building a 4WD Robot based on Arduino and Raspberry Pi

The goal of this project is to build a autonomous robot that can be expanded upon easily in software. The idea is to have a varietity of different sensors available from the start and build a nice looking casing around it.

## Update 20 April 2026!!

This has been 11 years since I've looked at the project. Times have changed and the advent of coding agents like GPT and Claude makes it easier to have multiple side-hobbies running. I will rebuild this whole project in Rust so I can learn low-level embedded programming and want to incorporate some of the advancements in AI to make it a more useful and fun robot.

I probably want to link this into my other hobby project [nexo](https://github.com/thiezn/nexo) that I'm working on to run models locally on my own hardware and provides a robust websocket API for inference and managing clients. 

My robot will then become another type of client to NEXO so it can do inference through my own setup. Moving all code to the old folder and starting fresh.

## Goals

- Autonomous driving robot
- Automatic object avoidance
- Facial recognition
- Video streaming
- Audio playback, Robot voice and music
- Voice control

## Hardware details

I've purchased the following initial hardware from the british site http://www.hobbycomponents.com/. It was the only site that I could find in Europe that had alot of the required components and had a lot of positive reviews. These parts will allow me to build the basic robot chassis for testing

- 1x Ultrasonic Module HC-SR04 Distance Sensor
- 2x L298N Stepper Motor Driver Controller Board
- 1x Hobby Components Arduino Compatible R3 Mega
- 1x V2 Mega Sensor Shield for Arduino
- 1x SG90 Pan & Titl servo bracket
- 2x Towerpro SG90 Micro servo 9g
- 1x 4 Wheeled Robot Smart Car Chassis Kit DC 3v 5v 6v suitable for Arduino
- 1x 20cm Solderless Female to Female DuPont Jumper Breadboard Wires (40-Cable Pack)

Next thing I need to take care of is the power supply. Initially I wanted to get a LiPo rechargable battery back with a USB circuit to recharge it. After some research I found out that I would need at least two LiPo backs in series (giving me 3.7x2 = 7.4V) but the available USB charger circuits would only work for single 3.7V packs. I decided on switching to LiPo 186500 batteries that I will install in a simple battery holder casing. I will have to remove the batteries from the holder to recharge them in a seperate charger. 

Purchasing batteries, a battery holder and battery charger in the Netherlands I found was very expensive. On recommendation from a collegue I placed an order at a chinese compagny called http://www.lightinthebox.com. I also purchased a bluetooth adapter and a ps3 compatible bluetooth controller that I want to use to remote control morTimmy the robot
- DIY 2-Slot 18650 Battery Holder with Pins – Black
- AC Charger + 2xUltraFire 18650 3.7V 3000mAh Rechargeable Battery with EU 100-240V Plug 
- Mini Bluetooth CSR V4.0 USB Dongle Adapter(Square)
- Rechargeable Bluetooth Wireless Controller for PS3 

The Raspberry Pi requires about 5V power and is sensitive to drops in voltage. Therefore to stabilise the voltage I decided to buy a so called UBEC. This is a switching voltage regulator that should be more efficient than the linear voltage regulator on the Raspberry Pi board itself. I purchased the following UBEC that came with pre-soldered micro-USB connector from the site http://dawnrobotics.co.uk
- 3A UBEC for Raspberry Pi

My intention is to implement voice control and audio playback into morTimmy. The raspberry does provide an stereo output jack but unfortunately no audio input. Also the power the output jack can provide is very low so I decided to purchase a low-cost USB audio interface for http://www.dx.com.
- USB 2.0 Virtual 7.1 Channel Surround Sound Card Caleb (20cm-Cable)

The last bit on my hardware kit list is a camera. I would like to use openCV on the Raspberry Pi so decided to purchase the official Raspberry Pi Camera module and the most recent Raspberry Pi model 2 B. This has a quad core processor which will allow me to use multiple threads on the different cores hopefully increasing performance dramatically over the previous Raspberry Pi models.
- Raspberry Pi 2 B
- Raspberry Pi Camera Module


## Project progress

This section outlines the current status of my project. 

What have I done so far:
- Setup (this) github repository, first time working with git so bear with me!
- Created skeleton classes for both Arduino (C++) and the Raspberry Pi (Python)
- Starting to use Docstrings to properly document my code. Trying to follow the
  Google Style Python Docstrings guidelines. Also implemented sphinx on my 
  home server to generate documentation from the Docstrings. The documentation
  can be found in the relevant docs/ directories and on my personal
  homepage http://morTimmy.mortimer.nl/
- Received the hardware order I've placed with http://www.hobbycomponents.com/

### Images of build process
  ![Screenshot of hardware ordered from hobbycomponents.com](http://raw.github.com/thiezn/morTimmy/master/images/hw_order.jpg)

### Hard- and Software Interfaces schematic

The following schematic shows the various interfaces between the hardware and software components of morTimmy the Robot. Keep in mind that these will likely change over time as the project develops.

  ![morTimmy Interfaces schematics] (http://raw.github.com/thiezn/morTimmy/master/images/morTimmy_Interfaces.png)

## Credits

- First have to credit my wife for putting up with me and my time consuming hobbies!

- Next I have to really give credit to Miguel Grindberg. He has created a really excellent tutorial on 
  building an Arduino Robot. I especially like his detailed explanation and he uses a proper Object Oriented 
  structure in his project which allows you to easily expand upon the code. Check it out here: 

  http://blog.miguelgrinberg.com/post/building-an-arduino-robot-part-i-hardware-components
