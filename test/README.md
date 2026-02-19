# Testing Readymade

This directory contains a simple test script and environment to verify that the Readymade backend works properly. It does the following:
1. Downloads an Ultramarine disk image, and sets up a target disk to install to.
2. Generates a Readymade playbook which gets loaded by the readymade-playbook helper, which executes the readymade backend and installs the operating system.
3. Uses isotovideo from OpenQA and the files in distri/ to boot the target disk and verify that the installation was successful.

## Running the test
Just cd into the test directory and run `./test.sh`.

## Notes
* Right now this test suite only tests one installation configuration, but it should be extended to support more partition schemes, distros, and postinstall modules.
