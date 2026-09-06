import ctypes as C
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time
import gi

gi.require_version('Gdk', '3.0')
from gi.repository import Gdk
root = Path(sys.argv[1])
evidence = {'case': 'DESKTOP-01', 'platform': 'Linux x86_64', 'toolchainPath': os.environ.get('AUDIT_DESKTOP_PATH', '/nonexistent'), 'packagedLocalMode': True}
app = None
try:
    executable = Path(sys.argv[2]) if len(sys.argv) > 2 else root / 'package/usr/bin/noema-desktop'
    env = dict(os.environ, PATH=evidence['toolchainPath'], NOEMA_HOME=str(root / 'home'), XDG_CONFIG_HOME=str(root / 'config'), XDG_CACHE_HOME=str(root / 'cache'))
    log = open(root / 'launch.log', 'w')
    app = subprocess.Popen([str(executable)], env=env, stdout=log, stderr=log)
    evidence['appPid'] = app.pid
    deadline = time.monotonic() + 40
    window = None
    while time.monotonic() < deadline and app.poll() is None:
        tree = subprocess.check_output(['/usr/bin/xwininfo', '-root', '-tree'], text=True)
        match = re.search(r'(0x[0-9a-f]+) "Noema"', tree)
        if match:
            window = int(match.group(1), 16)
            break
        time.sleep(0.2)
    assert window, 'The packaged Noema window did not open.'
    servers = []
    while time.monotonic() < deadline and app.poll() is None:
        children = set()
        for task in Path(f'/proc/{app.pid}/task').glob('*/children'):
            try:
                children.update(task.read_text().split())
            except FileNotFoundError:
                pass
        servers = []
        for pid in children:
            try:
                target = str(Path(f'/proc/{pid}/exe').resolve(strict=True))
            except FileNotFoundError:
                continue
            if target.endswith('/binaries/noema-server'):
                servers.append({'pid': int(pid), 'executable': target})
        if len(servers) == 1 and (root / 'home/noema.sqlite3').is_file():
            break
        time.sleep(0.2)
    assert len(servers) == 1, f'Expected one packaged Go server, found {len(servers)}.'
    evidence['server'] = servers[0]
    evidence['databaseCreated'] = (root / 'home/noema.sqlite3').is_file()
    assert evidence['databaseCreated']
    time.sleep(20)
    display = Gdk.Display.get_default()
    screen = display.get_default_screen()
    pixbuf = Gdk.pixbuf_get_from_window(screen.get_root_window(), 0, 0, screen.get_width(), screen.get_height())
    pixbuf.savev(str(root / 'desktop.png'), 'png', [], [])
    x = C.CDLL('libX11.so.6')
    x.XOpenDisplay.restype = C.c_void_p
    x.XInternAtom.argtypes = [C.c_void_p, C.c_char_p, C.c_int]
    x.XInternAtom.restype = C.c_ulong
    x.XSendEvent.argtypes = [C.c_void_p, C.c_ulong, C.c_int, C.c_long, C.c_void_p]
    x.XFlush.argtypes = [C.c_void_p]
    x.XCloseDisplay.argtypes = [C.c_void_p]
    connection = x.XOpenDisplay(None)
    class ClientMessage(C.Structure):
        _fields_ = [('type', C.c_int), ('serial', C.c_ulong), ('send_event', C.c_int), ('display', C.c_void_p), ('window', C.c_ulong), ('message_type', C.c_ulong), ('format', C.c_int), ('data', C.c_long * 5)]
    class Event(C.Union):
        _fields_ = [('client', ClientMessage), ('padding', C.c_long * 24)]
    event = Event()
    event.client = ClientMessage(33, 0, 1, connection, window, x.XInternAtom(connection, b'WM_PROTOCOLS', 0), 32, (C.c_long * 5)(x.XInternAtom(connection, b'WM_DELETE_WINDOW', 0), 0, 0, 0, 0))
    assert x.XSendEvent(connection, window, 0, 0, C.byref(event)) != 0
    x.XFlush(connection)
    evidence['closeMethod'] = 'WM_DELETE_WINDOW'
    evidence['exitCode'] = app.wait(timeout=20)
    evidence['serverStopped'] = not Path(f'/proc/{servers[0]["pid"]}').exists()
    assert evidence['exitCode'] == 0 and evidence['serverStopped']
    x.XCloseDisplay(connection)
except Exception as error:
    evidence['error'] = str(error)
    raise
finally:
    if app is not None and app.poll() is None:
        app.terminate()
        try:
            app.wait(timeout=15)
        except subprocess.TimeoutExpired:
            app.kill()
            app.wait()
    (root / 'launch-results.json').write_text(json.dumps(evidence, indent=2) + '\n')
