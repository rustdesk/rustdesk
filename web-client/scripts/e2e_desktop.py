#!/usr/bin/env python3
"""Full-screen X11 target used to prove remote video, clicks, and typing.

The window covers the virtual display. The top half is one button. The lower
band is a text field, and the bottom strip records a horizontal drag.
Every change is written to /tmp/rd-e2e-state.json.
"""

import json
import time
import tkinter as tk
from pathlib import Path

STATE = Path("/tmp/rd-e2e-state.json")
WIDTH = 1280
HEIGHT = 720


class Desktop:
    def __init__(self) -> None:
        self.clicks = 0
        self.text = ""
        self.drag = 0
        self.right = 0
        self.drag_origin: int | None = None
        self.root = tk.Tk()
        self.root.overrideredirect(True)
        self.root.geometry(f"{WIDTH}x{HEIGHT}+0+0")
        self.root.configure(bg="#f4f7fb")

        self.button = tk.Button(
            self.root,
            text=self.button_label(),
            command=self.on_click,
            font=("DejaVu Sans", 48, "bold"),
            bg="#0b6bcb",
            fg="white",
            activebackground="#0857a6",
            activeforeground="white",
            relief="flat",
            bd=0,
        )
        self.button.place(x=0, y=0, width=WIDTH, height=400)
        self.button.bind("<Button-3>", self.on_right)

        self.entry = tk.Entry(self.root, font=("DejaVu Sans", 32), bg="white", fg="#1c2430")
        self.entry.place(x=40, y=450, width=1200, height=72)
        self.entry.bind("<KeyRelease>", self.on_type)

        self.clock = tk.Label(
            self.root,
            text="",
            font=("DejaVu Sans", 22),
            bg="#f4f7fb",
            fg="#1c2430",
            anchor="w",
        )
        self.clock.place(x=40, y=560, width=1200, height=48)

        self.drag_label = tk.Label(
            self.root,
            text="DRAG 0",
            font=("DejaVu Sans", 22),
            bg="#e7eef6",
            fg="#1c2430",
        )
        self.drag_label.place(x=0, y=620, width=WIDTH, height=100)
        self.drag_label.bind("<Button-1>", self.on_drag_down)
        self.drag_label.bind("<B1-Motion>", self.on_drag_move)
        self.drag_label.bind("<ButtonRelease-1>", self.on_drag_up)
        self.root.bind("<ButtonRelease-1>", self.on_drag_up)
        self.write()
        self.tick()

    def button_label(self) -> str:
        return f"CLICKS: {self.clicks}"

    def on_click(self) -> None:
        self.clicks += 1
        self.button.configure(text=self.button_label())
        self.write()

    def on_right(self, _event: object) -> str:
        self.right += 1
        self.write()
        return "break"

    def on_type(self, _event: object) -> None:
        self.text = self.entry.get()
        self.write()

    def on_drag_down(self, event: tk.Event) -> None:
        self.drag_origin = int(event.x)

    def on_drag_move(self, event: tk.Event) -> None:
        if self.drag_origin is None:
            return
        self.drag = int(event.x) - self.drag_origin
        self.drag_label.configure(text=f"DRAG {self.drag}")
        self.write()

    def on_drag_up(self, event: tk.Event) -> None:
        if self.drag_origin is None:
            return
        self.drag = int(event.x) - self.drag_origin
        self.drag_origin = None
        self.drag_label.configure(text=f"DRAG {self.drag}")
        self.write()

    def tick(self) -> None:
        self.clock.configure(text=time.strftime("CLOCK %H:%M:%S"))
        self.root.after(200, self.tick)

    def write(self) -> None:
        payload = {"clicks": self.clicks, "text": self.text, "drag": self.drag, "right": self.right, "ts": time.time()}
        tmp = STATE.with_suffix(".json.tmp")
        tmp.write_text(json.dumps(payload), encoding="utf-8")
        tmp.replace(STATE)

    def run(self) -> None:
        self.root.mainloop()


if __name__ == "__main__":
    Desktop().run()
