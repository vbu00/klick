"""Иконки kl!ck из логотипа-клавиши (docs/logo, пять цветов на тёмном и
светлом фоне).

    python tools/make-icons.py

Форму берём из серой клавиши на тёмном фоне (docs/logo/grey-dark.png): она
ахроматическая, поэтому по яркости однозначно разделяется на фон (38),
корпус с «!» (140) и белую крышку (255) — вместе со сглаживанием краёв.
Дальше клавиша перекрашивается в любой цвет:

  src-tauri/icons/icon.png, icon.ico            — приложение и установщик (синяя, как вордмарк)
  src-tauri/icons/tray-{state}-N.png            — трей, по цвету на состояние
  src/assets/mark.png, mark-face.png            — знак для окна: корпус и крышка
                                                  отдельными масками (красятся CSS)
Нужны Pillow и numpy.
"""

import os

import numpy as np
from PIL import Image

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ICONS = os.path.join(ROOT, "src-tauri", "icons")
ASSETS = os.path.join(ROOT, "src", "assets")
SOURCE = os.path.join(ROOT, "docs", "logo", "grey-dark.png")

BG, BODY, FACE = 38.0, 140.0, 255.0

# Цвета логотипа (docs/logo) — по состоянию туннеля.
TRAY = {
    "ok": (34, 195, 138),     # зелёная — подключено
    "busy": (0, 122, 255),    # синяя — подключаюсь (мигает)
    "warn": (255, 157, 0),    # оранжевая — туннель есть, сервер не отвечает
    "bad": (234, 37, 86),     # красная — ошибка
    "idle": (140, 140, 140),  # серая — выключено
}
APP = TRAY["busy"]  # синяя — как в вордмарке (docs/logo/wordmark.svg)
SIZES = (16, 20, 24, 28, 32, 40, 48, 64)


def masks():
    """alpha — вся клавиша, face — доля белой крышки (0…1), обрезано по клавише."""
    v = np.asarray(Image.open(SOURCE).convert("L")).astype(np.float64)
    alpha = np.clip((v - BG) / (BODY - BG), 0, 1)
    face = np.clip((v - BODY) / (FACE - BODY), 0, 1)
    ys, xs = np.where(alpha > 0.02)
    cx, cy = (xs.min() + xs.max()) / 2, (ys.min() + ys.max()) / 2
    half = max(xs.max() - xs.min(), ys.max() - ys.min()) / 2 + 4
    y0, x0 = int(cy - half), int(cx - half)
    n = int(2 * half)
    return alpha[y0:y0 + n, x0:x0 + n], face[y0:y0 + n, x0:x0 + n]


def keycap(alpha, face, color, face_color=(255, 255, 255), opacity=1.0):
    """RGBA-клавиша в полном разрешении: корпус color, крышка face_color."""
    c, f = np.array(color, float), np.array(face_color, float)
    rgb = c[None, None, :] * (1 - face[..., None]) + f[None, None, :] * face[..., None]
    out = np.dstack([rgb, alpha * 255 * opacity]).round().astype(np.uint8)
    return Image.fromarray(out, "RGBA")


def sized(img, px, pad=0.0):
    """Уменьшить до px×px (Pillow учитывает альфу) с полями pad."""
    inner = max(1, round(px * (1 - 2 * pad)))
    small = img.resize((inner, inner), Image.LANCZOS)
    canvas = Image.new("RGBA", (px, px), (0, 0, 0, 0))
    off = (px - inner) // 2
    canvas.paste(small, (off, off), small)
    return canvas


def mask_png(a, px):
    """Маска для CSS (цвет берётся из currentColor): белое с альфой a."""
    img = Image.fromarray(np.dstack([np.full(a.shape + (3,), 255.0), a * 255]).round().astype(np.uint8), "RGBA")
    return img.resize((px, px), Image.LANCZOS)


def main():
    os.makedirs(ICONS, exist_ok=True)
    os.makedirs(ASSETS, exist_ok=True)
    alpha, face = masks()

    app = sized(keycap(alpha, face, APP), 512, pad=0.03)
    app.save(os.path.join(ICONS, "icon.png"))
    app.save(os.path.join(ICONS, "icon.ico"), sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])

    for state, rgb in TRAY.items():
        full = keycap(alpha, face, rgb)
        for px in SIZES:
            sized(full, px).save(os.path.join(ICONS, f"tray-{state}-{px}.png"))
    # Второй кадр мигания «подключаюсь» — та же синяя клавиша, приглушённая.
    dim = keycap(alpha, face, TRAY["busy"], opacity=0.45)
    for px in SIZES:
        sized(dim, px).save(os.path.join(ICONS, f"tray-busy2-{px}.png"))

    # Знак в окне: корпус (с «!») и крышка — две маски. CSS красит корпус
    # акцентом, крышку — белым: клавиша следует цвету акцента темы.
    mask_png(alpha * (1 - face), 128).save(os.path.join(ASSETS, "mark.png"))
    mask_png(face, 128).save(os.path.join(ASSETS, "mark-face.png"))
    print("готово")


if __name__ == "__main__":
    main()
