#!/bin/sh

exec env DISPLAY=$DISPLAY XAUTHORITY=$XAUTHORITY /usr/libexec/readymade "$@"
