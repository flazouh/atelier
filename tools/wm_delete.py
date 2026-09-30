#!/usr/bin/env python3
"""Asks an X window to close as a window manager's close button does: a WM_DELETE_WINDOW message."""
import ctypes, sys
x = ctypes.cdll.LoadLibrary("libX11.so.6")
x.XOpenDisplay.restype = ctypes.c_void_p
x.XInternAtom.restype = ctypes.c_ulong
x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int),
                ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong),
                ("format", ctypes.c_int), ("l", ctypes.c_long * 5)]
class Event(ctypes.Union):
    _fields_ = [("xclient", ClientMessage), ("pad", ctypes.c_long * 24)]
dpy = x.XOpenDisplay(None)
window = int(sys.argv[1])
protocols = x.XInternAtom(dpy, b"WM_PROTOCOLS", 0)
delete = x.XInternAtom(dpy, b"WM_DELETE_WINDOW", 0)
ev = Event()
ev.xclient.type = 33  # ClientMessage
ev.xclient.window = window
ev.xclient.message_type = protocols
ev.xclient.format = 32
ev.xclient.l[0] = delete
ev.xclient.l[1] = 0  # CurrentTime
x.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.POINTER(Event)]
x.XSendEvent(dpy, window, 0, 0, ctypes.byref(ev))
x.XFlush.argtypes = [ctypes.c_void_p]
x.XFlush(dpy)
