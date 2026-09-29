# Логотип kl!ck

Автор — [Dmitriy Medvedev](https://github.com/aleuuu).

<p>
  <img src="wordmark-light.svg" width="320" alt="kl!ck" />
</p>

Клавиша с «!» в пяти цветах, каждый — на тёмном и светлом фоне. В kl!ck
цвет клавиши в трее показывает состояние:

| Цвет | Файлы | Состояние |
|---|---|---|
| синий `#007AFF` | `blue-dark.png`, `blue-light.png` | подключаюсь (мигает); иконка приложения |
| зелёный `#22C38A` | `green-dark.png`, `green-light.png` | подключено |
| оранжевый `#FF9D00` | `orange-dark.png`, `orange-light.png` | туннель есть, сервер не отвечает |
| красный `#EA2556` | `red-dark.png`, `red-light.png` | ошибка |
| серый `#8C8C8C` | `grey-dark.png`, `grey-light.png` | выключено |

Вордмарк: `wordmark-banner.svg` — исходный, на белом; `wordmark-light.svg` и
`wordmark-dark.svg` — без фона, для светлой и тёмной темы; `keycap.svg` —
одна клавиша.

Иконки приложения и трея собирает `python tools/make-icons.py` (форму берёт
из `grey-dark.png`), картинки установщика — `bash tools/make-installer-art.sh`.
