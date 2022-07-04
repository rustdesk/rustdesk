from pynput.keyboard import Key, Controller
from pynput.keyboard._xorg import KeyCode
from pynput._util.xorg import display_manager
import Xlib
from pynput._util.xorg import *
import Xlib
import os
import sys
import socket

KeyCode._from_symbol("\0")  # test

DEAD_KEYS = {
    '`': 65104,
    '´': 65105,
    '^': 65106,
    '~': 65107,
    '¯': 65108,
    '˘': 65109,
    '˙': 65110,
    '¨': 65111,
    '˚': 65112,
    '˝': 65113,
    'ˇ': 65114,
    '¸': 65115,
    '˛': 65116,
    '℩': 65117,  # ?
    '゛': 65118,  # ?
    '゚ ': 65119,
    'ٜ': 65120,
    '↪': 65121,
    ' ̛': 65122,
}



def my_keyboard_mapping(display):
    """Generates a mapping from *keysyms* to *key codes* and required
    modifier shift states.

    :param Xlib.display.Display display: The display for which to retrieve the
        keyboard mapping.

    :return: the keyboard mapping
    """
    mapping = {}

    shift_mask = 1 << 0
    group_mask = alt_gr_mask(display)

    # Iterate over all keysym lists in the keyboard mapping
    min_keycode = display.display.info.min_keycode
    keycode_count = display.display.info.max_keycode - min_keycode + 1
    for index, keysyms in enumerate(display.get_keyboard_mapping(
            min_keycode, keycode_count)):
        key_code = index + min_keycode

        # Normalise the keysym list to yield a tuple containing the two groups
        normalized = keysym_normalize(keysyms)
        if not normalized:
            continue

        # Iterate over the groups to extract the shift and modifier state
        for groups, group in zip(normalized, (False, True)):
            for keysym, shift in zip(groups, (False, True)):

                if not keysym:
                    continue
                shift_state = 0 \
                    | (shift_mask if shift else 0) \
                    | (group_mask if group else 0)

                # !!!: Save all keycode combinations of keysym
                if keysym in mapping:
                    mapping[keysym].append((key_code, shift_state))
                else:
                    mapping[keysym] = [(key_code, shift_state)]
    return mapping


class MyController(Controller):
    def _update_keyboard_mapping(self):
        """Updates the keyboard mapping.
        """
        with display_manager(self._display) as dm:
            self._keyboard_mapping = my_keyboard_mapping(dm)

    def send_event(self, event, keycode, shift_state):
        with display_manager(self._display) as dm, self.modifiers as modifiers:
            # Under certain cimcumstances, such as when running under Xephyr,
            # the value returned by dm.get_input_focus is an int
            window = dm.get_input_focus().focus
            send_event = getattr(
                window,
                'send_event',
                lambda event: dm.send_event(window, event))
            send_event(event(
                detail=keycode,
                state=shift_state | self._shift_mask(modifiers),
                time=0,
                root=dm.screen().root,
                window=window,
                same_screen=0,
                child=Xlib.X.NONE,
                root_x=0, root_y=0, event_x=0, event_y=0))

    def fake_input(self, keycode, is_press):
        with display_manager(self._display) as dm:
            Xlib.ext.xtest.fake_input(
                dm,
                Xlib.X.KeyPress if is_press else Xlib.X.KeyRelease,
                keycode)

    def _handle(self, key, is_press):
        """Resolves a key identifier and sends a keyboard event.
        :param event: The *X* keyboard event.
        :param int keysym: The keysym to handle.
        """
        event = Xlib.display.event.KeyPress if is_press \
            else Xlib.display.event.KeyRelease
        keysym = self._keysym(key)

        if key.vk is not None:
            keycode = self._display.keysym_to_keycode(key.vk)
            self.fake_input(keycode, is_press)
            # Otherwise use XSendEvent; we need to use this in the general case to
            # work around problems with keyboard layouts
            self._emit('_on_fake_event', key, is_press)
            return

        # Make sure to verify that the key was resolved
        if keysym is None:
            raise self.InvalidKeyException(key)

        # There may be multiple keycodes for keysym in keyboard_mapping
        keycode_flag = len(self.keyboard_mapping[keysym]) == 1
        if keycode_flag:
            keycode, shift_state = self.keyboard_mapping[keysym][0]
        else:
            keycode, shift_state = self._display.keysym_to_keycode(keysym), 0

        keycode_set = set(map(lambda x: x[0], self.keyboard_mapping[keysym]))
        # The keycode of the dead key is inconsistent, The keysym has multiple combinations of a keycode.
        if keycode != self._display.keysym_to_keycode(keysym) \
            or (keycode_flag == False and keycode == list(keycode_set)[0] and len(keycode_set) == 1):
            deakkey_chr = str(key).replace("'", '')
            keysym = DEAD_KEYS[deakkey_chr]
            # shift_state = 0
            keycode, shift_state = list(
                filter(lambda x: x[1] == 0,
                       self.keyboard_mapping[keysym])
            )[0]

        # If the key has a virtual key code, use that immediately with
        # fake_input; fake input,being an X server extension, has access to
        # more internal state that we do

        try:
            with self.modifiers as modifiers:
                alt_gr = Key.alt_gr in modifiers
            # !!!: Send_event can't support lock screen, this condition cann't be modified
            if alt_gr:
                self.send_event(
                    event, keycode, shift_state)
            else:
                self.fake_input(keycode, is_press)
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
os.system('chmod a+rw %s' % server_address)
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
                print('[x] error key',e)


loop()
clientsocket.close()
server.close()
