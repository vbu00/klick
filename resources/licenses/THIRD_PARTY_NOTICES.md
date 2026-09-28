# Сторонние компоненты kl!ck

kl!ck — оболочка и служба. Сетевой туннель делает ядро **mihomo**; оно входит в kl!ck
отдельным исполняемым файлом без изменений.

## mihomo — ядро

- Репозиторий: https://github.com/MetaCubeX/mihomo
- Версия в kl!ck: v1.19.31, официальный выпуск для Windows (amd64)
- Лицензия: GNU General Public License v3.0 — полный текст в `GPL-3.0.txt` рядом с этим файлом
- Исходный код этой версии: https://github.com/MetaCubeX/mihomo/tree/v1.19.31

## Country.mmdb — база стран

- Источник: https://github.com/MetaCubeX/meta-rules-dat
- Лицензия набора данных: GNU General Public License v3.0 (`GPL-3.0.txt`)
- Файл поставляется без изменений; нужен переключателю «Российские IP напрямую».

## Окно и служба

- [Tauri](https://tauri.app), [React](https://react.dev) и библиотеки Rust (tokio, serde, reqwest, hyper, windows и др.) — MIT / Apache-2.0.
- Интерфейс, логотип и иконки — по макету, который сделал для kl!ck Dmitriy Medvedev (https://github.com/aleuuu).

Код самой kl!ck — лицензия MIT (`LICENSE.txt`).
