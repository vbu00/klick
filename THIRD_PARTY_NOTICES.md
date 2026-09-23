# Сторонние компоненты kl!ck

kl!ck — графическая оболочка. Весь сетевой туннель делает ядро **mihomo**;
в установщик kl!ck оно входит отдельным исполняемым файлом без изменений.

## mihomo — ядро

- Репозиторий: https://github.com/MetaCubeX/mihomo
- Версия в kl!ck: v1.19.31, сборка `mihomo-windows-amd64-compatible`
  (официальный релиз, sha256 `93d14e9a13b49b2f2d256202d02cc8d14a7c4695edf084cae0f941986bc9c218`)
- Лицензия: MIT

```
Copyright 2023 KT

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the “Software”), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## country.mmdb — GeoIP-база

- Источник: https://github.com/MetaCubeX/meta-rules-dat, ветка `release`,
  коммит `6b01e65c89cdeb684c00c70bb0b414091b22f124`
- Лицензия набора данных: GPL-3.0 (полный текст и исходные данные — в
  репозитории meta-rules-dat). Файл поставляется без изменений и отдельно от
  кода kl!ck; используется только пресетом «Российские IP — напрямую».

## Прочее

- [Tauri](https://tauri.app) и Rust-зависимости (serde, ureq, windows-sys и
  др.) — MIT / Apache-2.0; список с версиями — `src-tauri/Cargo.lock`.
- Интерфейс и иконки — по макету «VPN Client», сделанному для kl!ck.
- Разбор ссылок и часть системного кода перенесены из проектов автора
  [Klutz](https://github.com/vbu00) и KlutzBOX.
