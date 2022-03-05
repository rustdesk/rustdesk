from pynput.keyboard import Controller
import os
import sys
import socket

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
buf = []
while True:
    data = clientsocket.recv(1024)
    if not data:
        print("Connection broken")
        break
    buf.extend(data)
    n = buf[0]
    n = n + 1
    if len(buf) >= n:
        msg = bytearray(buf[1:n]).decode("utf-8")
        if len(msg) != 2:
            print("Wrong message")
            break
        if msg[0] == "p":
            keyboard.press(msg[1])
        else:
            keyboard.release(msg[1])
        buf = buf[n:]
clientsocket.close()
server.close()
