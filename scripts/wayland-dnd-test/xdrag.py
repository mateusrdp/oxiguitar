# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
#
# Drags with the mouse on X display :99 through XTEST: press at the first
# point, move to the second, release. Usage: xdrag.py X0,Y0 X1,Y1
import sys, time
from Xlib import X, display
from Xlib.ext import xtest
d = display.Display(':99')
(x0, y0), (x1, y1) = [tuple(map(int, a.split(','))) for a in sys.argv[1:3]]
def move(x, y):
    xtest.fake_input(d, X.MotionNotify, x=x, y=y); d.sync(); time.sleep(0.05)
move(x0, y0); time.sleep(0.3)
xtest.fake_input(d, X.ButtonPress, 1); d.sync(); time.sleep(0.3)
for i in range(1, 21):
    move(x0 + (x1 - x0) * i // 20, y0 + (y1 - y0) * i // 20)
time.sleep(0.5)
xtest.fake_input(d, X.ButtonRelease, 1); d.sync()
