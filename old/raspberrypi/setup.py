#!/usr/bin/env python

from distutils.core import setup

setup(name='morTimmy',
      version='0.1',
      description='morTimmy the robot',
      author='Mathijs Mortimer',
      author_email='mathijs@mortimer.nl',
      url='https://github.com/thiezn/morTimmy',
      packages=['morTimmy'],
      install_requires=[
          'pyserial>=2.7',
	  'pybluez>=0.20'
          ]
      )
