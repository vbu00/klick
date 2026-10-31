#!/usr/bin/env python3
"""Значки веб-версии (ui/src/assets/icons, status) → векторные значки Android (res/drawable/k_*.xml).

Значки — только контуры <path> с fill: `currentColor` (цвет задаёт интерфейс, как у веб-версии и iOS) или
цвет (значки состояния — клавиша kl!ck). viewBox со сдвигом — через группу с переносом. Другое (градиенты,
фигуры) — ошибка: лучше узнать при сборке, чем увидеть пустой значок.

    python3 scripts/android/icons.py ui/src/assets crates/klick-android/app/src/main/res/drawable
"""
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

SVG = '{http://www.w3.org/2000/svg}'


def color(value: str) -> str:
    value = value.strip()
    if value in ('currentColor', '', 'white'):
        return '#FFFFFFFF'
    if value == 'black':
        return '#FF000000'
    m = re.fullmatch(r'#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})', value)
    if not m:
        raise ValueError(f'цвет {value!r} не поддерживается')
    hexv = m.group(1)
    if len(hexv) == 3:
        hexv = ''.join(c * 2 for c in hexv)
    return '#FF' + hexv.upper()


def convert(svg: Path) -> str:
    root = ET.parse(svg).getroot()
    x, y, w, h = (float(v) for v in root.get('viewBox', '0 0 24 24').replace(',', ' ').split())
    paths = []
    for el in root.iter():
        tag = el.tag.replace(SVG, '')
        if tag in ('svg', 'g', 'title', 'desc'):
            continue
        if tag != 'path':
            raise ValueError(f'{svg.name}: элемент <{tag}> не поддерживается')
        fill = el.get('fill', root.get('fill', 'currentColor'))
        if fill == 'none':
            continue
        attrs = [f'android:pathData="{el.get("d")}"', f'android:fillColor="{color(fill)}"']
        if el.get('fill-rule') == 'evenodd':
            attrs.append('android:fillType="evenOdd"')
        if el.get('fill-opacity'):
            attrs.append(f'android:fillAlpha="{el.get("fill-opacity")}"')
        paths.append('        <path ' + ' '.join(attrs) + ' />')
    size = 24
    body = '\n'.join(paths)
    group_open = f'    <group android:translateX="{-x:g}" android:translateY="{-y:g}">' if (x or y) else '    <group>'
    return (
        '<?xml version="1.0" encoding="utf-8"?>\n'
        f'<!-- Создан scripts/android/icons.py из {svg.parent.name}/{svg.name} — не править руками. -->\n'
        '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
        f'    android:width="{size}dp" android:height="{size * h / w:g}dp"\n'
        f'    android:viewportWidth="{w:g}" android:viewportHeight="{h:g}">\n'
        f'{group_open}\n{body}\n    </group>\n</vector>\n'
    )


def main() -> None:
    src, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob('k_*.xml'):
        old.unlink()
    count = 0
    for folder, prefix in (('icons', 'k_'), ('status', 'k_status_')):
        for svg in sorted((src / folder).glob('*.svg')):
            name = prefix + re.sub(r'[^a-z0-9]', '_', svg.stem.lower())
            (out / f'{name}.xml').write_text(convert(svg), encoding='utf-8')
            count += 1
    print(f'значков: {count}')


if __name__ == '__main__':
    main()
