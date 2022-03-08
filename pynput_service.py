from pynput.keyboard import Key, Controller
from pynput.keyboard._xorg import KeyCode
import os
import sys
import socket

INVALID = KeyCode._from_symbol("\0")  # test

keyboard = Controller()

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
                keyboard = Controller()
                print("Keyboard reset")
                continue
            if len(msg) == 2:
                name = msg[1]
            else:
                name = KeyCode._from_symbol(msg[1:])
            if name == INVALID:
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
