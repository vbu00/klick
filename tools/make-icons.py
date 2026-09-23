"""Иконки kl!ck: знак — «!» цвета акцента (капсула и точка).

    python tools/make-icons.py

Кладёт:
  src-tauri/icons/icon.png, icon.ico         — приложение и установщик
  src-tauri/icons/tray-{ok,warn,warn2,bad,idle}-N — трей (плашка, знак вырезан)
  src/assets/mark.png                        — знак в титулбаре окна
Рисуем в 8 раз крупнее и уменьшаем — края чистые. Нужен Pillow.
"""

import os

from PIL import Image, ImageChops, ImageDraw

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ICONS = os.path.join(ROOT, "src-tauri", "icons")
ASSETS = os.path.join(ROOT, "src", "assets")

ACCENT = (48, 209, 88)      # #30d158 — акцент макета
WIN = (26, 26, 29)          # #1a1a1d — фон окна
TRAY = {
    "ok": ACCENT,
    "warn": (255, 179, 64),  # #ffb340
    "bad": (255, 105, 97),   # #ff6961
    "idle": (142, 142, 147), # #8e8e93
}
S = 8


def mark(size, color, plate=None):
    """Знак «!» в квадрате size×size; plate — цвет подложки-скругления."""
    big = size * S
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    if plate:
        d.rounded_rectangle([0, 0, big - 1, big - 1], radius=big * 0.23, fill=plate + (255,))
        scale = 0.62  # знак на подложке — с полями
    else:
        scale = 0.98
    h = big * scale
    top = (big - h) / 2
    w = h * 0.30                       # ширина капсулы и точки
    cx = big / 2
    bar_h = h * 0.64
    d.rounded_rectangle([cx - w / 2, top, cx + w / 2, top + bar_h], radius=w / 2, fill=color + (255,))
    dot_top = top + h - w
    d.ellipse([cx - w / 2, dot_top, cx + w / 2, dot_top + w], fill=color + (255,))
    return img.resize((size, size), Image.LANCZOS)


def plate(size, color, alpha=255):
    """Иконка трея: скруглённая плашка цвета состояния, «!» вырезан насквозь —
    на 16 px читается лучше тонкого знака и не теряется ни на светлой, ни на
    тёмной панели задач."""
    big = size * S
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    pad = big * 0.03
    d.rounded_rectangle([pad, pad, big - 1 - pad, big - 1 - pad], radius=big * 0.3, fill=color + (alpha,))
    cut = mark(size, (0, 0, 0), None).resize((big, big), Image.LANCZOS)  # тот же знак
    # Знак поменьше плашки — с полями.
    k = 0.58
    small = cut.resize((int(big * k), int(big * k)), Image.LANCZOS)
    hole = Image.new("L", (big, big), 0)
    hole.paste(small.getchannel("A"), (int(big * (1 - k) / 2), int(big * (1 - k) / 2)))
    img.putalpha(ImageChops.subtract(img.getchannel("A"), hole))
    return img.resize((size, size), Image.LANCZOS)


def main():
    os.makedirs(ICONS, exist_ok=True)
    os.makedirs(ASSETS, exist_ok=True)
    app = mark(512, ACCENT, WIN)
    app.save(os.path.join(ICONS, "icon.png"))
    app.save(os.path.join(ICONS, "icon.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    for state, rgb in TRAY.items():
        for px in (16, 20, 24, 28, 32, 40, 48, 64):
            plate(px, rgb).save(os.path.join(ICONS, f"tray-{state}-{px}.png"))
            # Второй кадр мигания «подключаюсь».
            if state == "warn":
                plate(px, rgb, 110).save(os.path.join(ICONS, f"tray-warn2-{px}.png"))
    mark(64, ACCENT).save(os.path.join(ASSETS, "mark.png"))
    app.resize((128, 128), Image.LANCZOS).save(os.path.join(ASSETS, "app-128.png"))
    print("готово")


if __name__ == "__main__":
    main()
