"""E6.1 spike: an app that was running before the font arrived.

Polls libfontconfig the way a long-lived app does (FcInitBringUptoDate, which
honours the config's rescan interval, 30 s by default) and prints how many ms
passed until the family showed up.

    python3 linux-watch.py <family> <ready-file> <seconds>
"""

import ctypes
import json
import sys
import time

fc = ctypes.CDLL("libfontconfig.so.1")
fc.FcNameParse.restype = ctypes.c_void_p
fc.FcNameParse.argtypes = [ctypes.c_char_p]
fc.FcObjectSetCreate.restype = ctypes.c_void_p
fc.FcObjectSetAdd.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
fc.FcObjectSetDestroy.argtypes = [ctypes.c_void_p]
fc.FcFontList.restype = ctypes.c_void_p
fc.FcFontList.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
fc.FcFontSetDestroy.argtypes = [ctypes.c_void_p]
fc.FcPatternDestroy.argtypes = [ctypes.c_void_p]


class FcFontSet(ctypes.Structure):
    _fields_ = [("nfont", ctypes.c_int), ("sfont", ctypes.c_int), ("fonts", ctypes.c_void_p)]


def present(family: str) -> bool:
    fc.FcInitBringUptoDate()
    pat = fc.FcNameParse(f":family={family}".encode())
    os_ = fc.FcObjectSetCreate()  # not FcObjectSetBuild: variadic, unsafe through ctypes
    fc.FcObjectSetAdd(os_, b"family")
    fs = fc.FcFontList(None, pat, os_)
    n = FcFontSet.from_address(fs).nfont
    fc.FcFontSetDestroy(fs)
    fc.FcPatternDestroy(pat)
    fc.FcObjectSetDestroy(os_)
    return n > 0


family, ready, seconds = sys.argv[1], sys.argv[2], float(sys.argv[3])
fc.FcInit()
open(ready, "w").write("up")
start = time.monotonic()
seen = None
while seen is None and time.monotonic() - start < seconds:
    if present(family):
        seen = int((time.monotonic() - start) * 1000)
    time.sleep(0.25)
print(json.dumps({"seen_ms": seen}))
