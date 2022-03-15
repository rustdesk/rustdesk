from pynput.keyboard import Key, Controller
from pynput.keyboard._xorg import KeyCode
from pynput._util.xorg import display_manager
import Xlib
import os
import sys
import socket

KeyCode._from_symbol("\0")  # test

class MyController(Controller):
    def _handle(self, key, is_press):
        """Resolves a key identifier and sends a keyboard event.
        :param event: The *X* keyboard event.
        :param int keysym: The keysym to handle.
        """
        event = Xlib.display.event.KeyPress if is_press \
            else Xlib.display.event.KeyRelease
        keysym = self._keysym(key)

        # Make sure to verify that the key was resolved
        if keysym is None:
            raise self.InvalidKeyException(key)

        # If the key has a virtual key code, use that immediately with
        # fake_input; fake input,being an X server extension, has access to
        # more internal state that we do
        if key.vk is not None:
            with display_manager(self._display) as dm:
                Xlib.ext.xtest.fake_input(
                    dm,
                    Xlib.X.KeyPress if is_press else Xlib.X.KeyRelease,
                    dm.keysym_to_keycode(key.vk))

        # Otherwise use XSendEvent; we need to use this in the general case to
        # work around problems with keyboard layouts
        else:
            try:
                keycode, shift_state = self.keyboard_mapping[keysym]
                with self.modifiers as modifiers:
                    alt_gr = Key.alt_gr in modifiers
                if alt_gr:
                    self._send_key(event, keycode, shift_state)
                else:
                    with display_manager(self._display) as dm:
                        Xlib.ext.xtest.fake_input(
                            dm,
                            Xlib.X.KeyPress if is_press else Xlib.X.KeyRelease,
                            keycode)

            except KeyError:
                with self._borrow_lock:
                    keycode, index, count = self._borrows[keysym]
                    self._send_key(
                        event,
                        keycode,
                        index_to_shift(self._display, index))
                    count += 1 if is_press else -1
                    self._borrows[keysym] = (keycode, index, count)

        # Notify any running listeners
        self._emit('_on_fake_event', key, is_press)

keyboard = MyController()

server_address = sys.argv[1]
if not os.path.exists(os.path.dirname(server_address)):
    os.makedirs(os.path.dirname(server_address))

try:
    os.unlink(server_address)
except OSError:
    if os.path.exists(server_address):
        raise

server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
server.bind(server_address)
server.listen(1)
clientsocket, address = server.accept()
os.system('chmod a+rw %s'%server_address)
print("Got pynput connection")


def loop():
    global keyboard
    buf = []
    while True:
        data = clientsocket.recv(1024)
        if not data:
            print("Connection broken")
            break
        buf.extend(data)
        while buf:
            n = buf[0]
            n = n + 1
            if len(buf) < n:
                break
            msg = bytearray(buf[1:n]).decode("utf-8")
            buf = buf[n:]
            if len(msg) < 2:
                continue
            if msg[1] == "\0":
                keyboard = MyController()
                print("Keyboard reset")
                continue
            if len(msg) == 2:
                name = msg[1]
            else:
                name = KeyCode._from_symbol(msg[1:])
            if str(name) == "<0>":
                continue
            try:
                if msg[0] == "p":
                    keyboard.press(name)
                else:
                    keyboard.release(name)
            except Exception as e:
                print(e)


loop()
clientsocket.close()
server.close()

