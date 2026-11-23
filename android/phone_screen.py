#!/usr/bin/env python3
"""Экран Android в окне Windows: живой просмотр и управление через adb.

Замена scrcpy для случаев, когда его нет под рукой: каждые ~0,3 с снимает
`screencap`, показывает картинку в окне Tk и превращает мышь в `input tap`
и `input swipe`. Клики и снимки после них складываются в отдельную папку,
чтобы потом можно было посмотреть, что именно нажимали.

    python android/phone_screen.py
    python android/phone_screen.py --serial R3CX20H54YZ --height 900

Мышь:  левый клик — касание, протяжка — свайп,
       правый клик — «Назад», средний — «Домой», колесо — прокрутка.
Клавиши: Esc — «Назад», Home — «Домой», F5 — перерисовать,
         Ctrl+S — сохранить снимок, Ctrl+Q — закрыть.
"""

from __future__ import annotations

import argparse
import ctypes
import io
import os
import queue
import re
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime
from pathlib import Path

try:
    from PIL import Image, ImageTk
except ImportError:  # pragma: no cover - подсказка вместо трейсбека
    sys.exit("Нужен Pillow: python -m pip install pillow")

import tkinter as tk
from tkinter import messagebox

CREATE_NO_WINDOW = 0x08000000 if os.name == "nt" else 0


def find_adb() -> str:
    """adb из SDK, из PATH или рядом со скриптом."""
    local = os.environ.get("LOCALAPPDATA")
    if local:
        p = Path(local) / "Android" / "Sdk" / "platform-tools" / "adb.exe"
        if p.is_file():
            return str(p)
    found = shutil.which("adb")
    if found:
        return found
    return "adb"


def devices(adb: str) -> list[str]:
    try:
        out = subprocess.run(
            [adb, "devices"],
            capture_output=True,
            text=True,
            timeout=15,
            creationflags=CREATE_NO_WINDOW,
        ).stdout
    except (OSError, subprocess.TimeoutExpired) as e:
        raise RuntimeError(f"adb не запустился: {e}") from e
    result = []
    for line in out.splitlines()[1:]:
        parts = line.split()
        if len(parts) >= 2 and parts[1] == "device":
            result.append(parts[0])
    return result


class Phone:
    """Один телефон: снимки экрана и команды ввода."""

    def __init__(self, adb: str, serial: str):
        self.adb = adb
        self.serial = serial
        self._lock = threading.Lock()

    def _run(self, args: list[str], timeout: float = 30) -> subprocess.CompletedProcess:
        with self._lock:
            return subprocess.run(
                [self.adb, "-s", self.serial, *args],
                capture_output=True,
                timeout=timeout,
                creationflags=CREATE_NO_WINDOW,
            )

    def screencap(self) -> Image.Image:
        p = self._run(["exec-out", "screencap", "-p"], timeout=30)
        data = p.stdout
        if not data.startswith(b"\x89PNG"):
            err = p.stderr.decode("utf-8", "replace").strip()
            raise RuntimeError(err or "пустой снимок экрана")
        return Image.open(io.BytesIO(data)).convert("RGB")

    def key(self, code: str) -> None:
        self._run(["shell", "input", "keyevent", code])

    def tap(self, x: int, y: int) -> None:
        self._run(["shell", "input", "tap", str(x), str(y)])

    def swipe(self, x1: int, y1: int, x2: int, y2: int, ms: int) -> None:
        self._run(
            ["shell", "input", "swipe", str(x1), str(y1), str(x2), str(y2), str(ms)]
        )

    def text(self, s: str) -> None:
        # input text не понимает пробелы и часть символов без экранирования
        safe = s.replace(" ", "%s")
        self._run(["shell", "input", "text", safe])


class Mirror:
    def __init__(self, phone: Phone, height: int, shots: Path):
        self.phone = phone
        self.shots = shots
        self.shots.mkdir(parents=True, exist_ok=True)
        self.log = (shots / "actions.log").open("a", encoding="utf-8")

        self.frames: queue.Queue[Image.Image] = queue.Queue(maxsize=1)
        self.stop = threading.Event()
        self.want_shot: str | None = None
        self.shot_no = 0
        self.latest: Image.Image | None = None
        self.view = (0, 0, 0.0, 0.0)  # ox, oy, scale, ширина кадра
        self.press: tuple[int, int, float] | None = None
        self.last_action = ""
        self.captures = 0
        self.fail = ""

        self.root = tk.Tk()
        self.root.title("GrazySAI — экран телефона")
        self.root.configure(bg="#111")
        self.root.protocol("WM_DELETE_WINDOW", self.close)

        bar = tk.Frame(self.root, bg="#1c1c1c")
        bar.pack(side=tk.TOP, fill=tk.X)
        for text, cmd in (
            ("Назад", lambda: self.act("back", lambda: self.phone.key("KEYCODE_BACK"))),
            ("Домой", lambda: self.act("home", lambda: self.phone.key("KEYCODE_HOME"))),
            ("Недавние", lambda: self.act("recents", lambda: self.phone.key("KEYCODE_APP_SWITCH"))),
            ("Снимок", self.save_now),
            ("Обновить", self.refresh),
        ):
            tk.Button(
                bar,
                text=text,
                command=cmd,
                bg="#333",
                fg="#eee",
                activebackground="#444",
                activeforeground="#fff",
                relief=tk.FLAT,
                padx=8,
                pady=3,
            ).pack(side=tk.LEFT, padx=2, pady=3)

        self.canvas = tk.Canvas(self.root, bg="#111", highlightthickness=0)
        self.canvas.pack(side=tk.TOP, fill=tk.BOTH, expand=True)
        self.image_item = self.canvas.create_image(0, 0, anchor=tk.NW)
        self.status = tk.Label(
            self.root, text="Подключение…", bg="#1c1c1c", fg="#aaa", anchor=tk.W
        )
        self.status.pack(side=tk.BOTTOM, fill=tk.X)

        self.canvas.bind("<Button-1>", self.on_press)
        self.canvas.bind("<B1-Motion>", self.on_move)
        self.canvas.bind("<ButtonRelease-1>", self.on_release)
        self.canvas.bind("<Button-3>", lambda e: self.act("back", lambda: self.phone.key("KEYCODE_BACK")))
        self.canvas.bind("<Button-2>", lambda e: self.act("home", lambda: self.phone.key("KEYCODE_HOME")))
        self.canvas.bind("<MouseWheel>", self.on_wheel)
        self.root.bind("<Escape>", lambda e: self.act("back", lambda: self.phone.key("KEYCODE_BACK")))
        self.root.bind("<Home>", lambda e: self.act("home", lambda: self.phone.key("KEYCODE_HOME")))
        self.root.bind("<F5>", lambda e: self.refresh())
        self.root.bind("<Control-s>", lambda e: self.save_now())
        self.root.bind("<Control-q>", lambda e: self.close())

        sw, sh = self.root.winfo_screenwidth(), self.root.winfo_screenheight()
        win_h = max(480, min(height, sh - 120))
        win_w = int(win_h * 0.52) + 20
        self.root.geometry(f"{win_w}x{win_h}+{max(0, sw - win_w - 40)}+40")

        threading.Thread(target=self.poll, daemon=True).start()
        self.root.after(60, self.tick)

    # ---------- кадры ----------

    def poll(self) -> None:
        while not self.stop.is_set():
            started = time.monotonic()
            try:
                im = self.phone.screencap()
                self.fail = ""
            except Exception as e:  # телефон отключили, adb занят и т. п.
                self.fail = str(e)
                self.stop.wait(1.0)
                continue
            self.captures += 1
            try:
                while True:
                    self.frames.get_nowait()
            except queue.Empty:
                pass
            self.frames.put(im)
            # не чаще ~3 кадров/с: screencap на телефоне небыстрый
            self.stop.wait(max(0.05, 0.33 - (time.monotonic() - started)))

    def tick(self) -> None:
        if self.stop.is_set():
            return
        try:
            im = self.frames.get_nowait()
        except queue.Empty:
            im = None
        if im is not None:
            self.latest = im
            self.draw(im)
            if self.want_shot:
                label, self.want_shot = self.want_shot, None
                try:
                    self.save(im, label)
                except OSError as e:
                    self.fail = f"снимок не сохранён: {e}"
        self.root.after(60, self.tick)

    def draw(self, im: Image.Image) -> None:
        cw = max(1, self.canvas.winfo_width())
        ch = max(1, self.canvas.winfo_height())
        scale = min(cw / im.width, ch / im.height)
        w, h = max(1, int(im.width * scale)), max(1, int(im.height * scale))
        ox, oy = (cw - w) // 2, (ch - h) // 2
        self.view = (ox, oy, scale, im.width)
        shown = im.resize((w, h), Image.LANCZOS)
        self.photo = ImageTk.PhotoImage(shown)
        self.canvas.itemconfigure(self.image_item, image=self.photo)
        self.canvas.coords(self.image_item, ox, oy)
        text = f"{im.width}×{im.height} · {self.captures} кадров"
        if self.last_action:
            text += f" · {self.last_action}"
        if self.fail:
            text += f" · ОШИБКА: {self.fail}"
        self.status.configure(text=text)

    def to_phone(self, x: int, y: int) -> tuple[int, int] | None:
        ox, oy, scale, _ = self.view
        if scale <= 0 or not self.latest:
            return None
        px = int((x - ox) / scale)
        py = int((y - oy) / scale)
        if 0 <= px < self.latest.width and 0 <= py < self.latest.height:
            return px, py
        return None

    # ---------- ввод ----------

    def act(self, name: str, fn) -> None:
        try:
            fn()
            self.note(name)
        except Exception as e:
            self.fail = str(e)

    def note(self, name: str) -> None:
        stamp = datetime.now().strftime("%H:%M:%S")
        self.last_action = name
        self.log.write(f"{stamp}\t{name}\n")
        self.log.flush()
        self.want_shot = name.replace(" ", "_")[:40]

    def on_press(self, e) -> None:
        p = self.to_phone(e.x, e.y)
        if p:
            self.press = (p[0], p[1], time.monotonic())

    def on_move(self, e) -> None:
        # координаты нужны только при отпускании; движение не шлём,
        # иначе каждый пиксель стал бы отдельным свайпом
        pass

    def on_release(self, e) -> None:
        if not self.press:
            return
        x1, y1, t0 = self.press
        self.press = None
        end = self.to_phone(e.x, e.y)
        if not end:
            return
        x2, y2 = end
        dist = ((x2 - x1) ** 2 + (y2 - y1) ** 2) ** 0.5
        if dist < 12:
            self.act(f"tap {x2},{y2}", lambda: self.phone.tap(x2, y2))
        else:
            ms = int(max(120, min(800, dist * 2)))
            self.act(
                f"swipe {x1},{y1}->{x2},{y2}",
                lambda: self.phone.swipe(x1, y1, x2, y2, ms),
            )

    def on_wheel(self, e) -> None:
        if not self.latest:
            return
        x, y = self.latest.width // 2, self.latest.height // 2
        step = self.latest.height // 4
        if e.delta > 0:  # колесо вверх — содержимое вверх
            self.act("wheel up", lambda: self.phone.swipe(x, y - step, x, y + step, 250))
        else:
            self.act("wheel down", lambda: self.phone.swipe(x, y + step, x, y - step, 250))

    # ---------- снимки ----------

    def save(self, im: Image.Image, label: str) -> None:
        self.shot_no += 1
        # в имени файла не должно быть <>:"/\|?* — иначе Windows не сохранит снимок
        safe = re.sub(r"[^A-Za-z0-9_.-]+", "_", label).strip("_")[:40] or "shot"
        name = f"{self.shot_no:03d}_{safe}.png"
        path = self.shots / name
        im.save(path)
        print(f"снимок: {path}")

    def save_now(self) -> None:
        if not self.latest:
            return
        try:
            self.save(self.latest, "manual")
            self.last_action = "снимок сохранён"
        except OSError as e:
            self.fail = f"снимок не сохранён: {e}"

    def refresh(self) -> None:
        self.last_action = "обновление…"

    def close(self) -> None:
        self.stop.set()
        try:
            self.log.close()
        except Exception:
            pass
        self.root.destroy()


def main() -> int:
    ap = argparse.ArgumentParser(description="Экран Android в окне Windows")
    ap.add_argument("--serial", help="серийный номер устройства (adb devices)")
    ap.add_argument("--adb", default=find_adb(), help="путь к adb")
    ap.add_argument("--height", type=int, default=900, help="высота окна, px")
    ap.add_argument(
        "--shots",
        default=str(Path(os.environ.get("TEMP", ".")) / "opencode" / "phone_shots"),
        help="куда складывать снимки и actions.log",
    )
    args = ap.parse_args()

    if os.name == "nt":
        try:
            ctypes.windll.shcore.SetProcessDpiAwareness(1)
        except Exception:
            pass

    try:
        found = devices(args.adb)
    except RuntimeError as e:
        print(e, file=sys.stderr)
        return 1
    if not found:
        print("Телефон не найден. Включите отладку по USB и разрешите доступ.", file=sys.stderr)
        return 1
    serial = args.serial or (found[0] if len(found) == 1 else None)
    if not serial:
        print(f"Подключено несколько устройств: {', '.join(found)}", file=sys.stderr)
        print("Укажите нужное: --serial <номер>", file=sys.stderr)
        return 1
    if serial not in found:
        print(f"Устройство {serial} не в списке adb: {', '.join(found)}", file=sys.stderr)
        return 1

    phone = Phone(args.adb, serial)
    try:
        test = phone.screencap()
    except Exception as e:
        print(f"Не удалось снять экран: {e}", file=sys.stderr)
        return 1
    print(f"Устройство {serial}, экран {test.width}x{test.height}")
    print(f"Снимки и журнал: {args.shots}")

    mirror = Mirror(phone, args.height, Path(args.shots))
    # стартовый кадр, чтобы окно сразу было не пустым
    mirror.latest = test
    mirror.draw(test)
    mirror.save(test, "start")
    try:
        mirror.root.mainloop()
    except KeyboardInterrupt:
        mirror.close()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
