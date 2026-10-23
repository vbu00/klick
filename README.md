<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/logo/wordmark-dark.svg" />
  <img src="docs/logo/wordmark-light.svg" width="360" alt="kl!ck" />
</picture>

**VPN-клиент для Windows, macOS и iOS на ядре [mihomo](https://github.com/MetaCubeX/mihomo)**

Подписки Remnawave · Marzban · 3x-ui · VLESS/Reality · Hysteria2 · TUIC · Trojan · Shadowsocks · WireGuard

[Скачать](https://github.com/limeflash/klick/releases/latest) · [Что в этой сборке](#что-в-этой-сборке) · [Что умеет](#что-умеет) · [Установка](#установка) · [macOS](#macos) · [iOS](#ios) · [Сборка](#сборка) · [Авторы](#авторы) · [Лицензия](#лицензия)

</div>

---

## Что в этой сборке

Это [limeflash/klick](https://github.com/limeflash/klick) — форк [vbu00/klick](https://github.com/vbu00/klick):
kl!ck для Windows, перенесённый на macOS, iPhone и iPad, плюс исправления стабильности. Сделанное здесь
уходит в vbu00/klick пулл-реквестами: порт на macOS уже там, версия 0.5 — в
[vbu00/klick#12](https://github.com/vbu00/klick/pull/12), iOS пока есть только здесь.

| | Windows | macOS | iPhone и iPad |
|---|---|---|---|
| **Скачать** | [установщик `.exe`](https://github.com/limeflash/klick/releases/latest) | [пакет `.pkg`](https://github.com/limeflash/klick/releases/latest/download/klick-macos.pkg) | [приложение `.ipa`](https://github.com/limeflash/klick/releases/latest/download/klick-ios.ipa) |
| **Система** | Windows 10/11 x64 | macOS 12+, Apple Silicon и Intel | iOS 15+ |
| **VPN держит** | служба Windows | служба launchd | системное расширение VPN |
| **Kill Switch** | фильтры Windows (WFP) | брандмауэр pf | — |
| **Как поставить** | [установщик](#установка) | [из Терминала](#macos) | [подписать и поставить](https://github.com/limeflash/klick/blob/ios/docs/ios-install.md) |

**iPhone и iPad — новое.** Тот же kl!ck: подписки, серверы по странам, «VPN для всего / для выбранного»,
проверка серверов, темы. VPN держит системное расширение, поэтому он работает и с закрытым kl!ck,
включается в Настройках → VPN и, если попросить, сам поднимается после перезагрузки. Ядро mihomo собрано из
исходников прямо в расширение и настроено под лимит памяти iOS. Подписку можно добавить по QR-коду — камерой или из фото. Есть
источник для AltStore, SideStore, Feather и ESign: kl!ck появляется в их списке и обновляется оттуда.

**macOS.** То же окно и те же функции, что на Windows: служба, TUN и системный прокси, Kill Switch, ссылки
`klick://`, обновление пакетом поверх. DNS и прокси возвращаются как были — при смене «Размещения»,
после сбоя и при удалении.

**VPN не переподключается сам (0.5).** Одна неудачная проверка сервера больше ничего не трогает: kl!ck
перепроверяет молча и переподключает, только если связи нет 20 секунд. Смена сети — только когда меняется
выход в интернет, после сна связь проверяется сразу. Добавили сайт или программу в список — загрузки и
звонки не рвутся. Смена сервера срабатывает сразу. Отказ панели или пустая подписка не стирают рабочие
серверы и не пускают трафик мимо VPN.

**Проверка серверов из своей сети (0.5).** Какие серверы работают, какие провайдер блокирует, а какие
«замораживает» ТСПУ (сервер отвечает, а загрузка встаёт), подменяет ли он DNS — и на какой сервер перейти.
Итоги можно отправлять в панель Mistgate: по согласию, через VPN, без IP и имени устройства.

**Каждое изменение проверяет CI** на Windows Server 2022/2025, macOS 15 и в симуляторе iOS: служба,
установщик, Kill Switch, страж под сбоями сети, конфиги на настоящем ядре mihomo, подключение через
серверы Hysteria2 и VLESS Reality.

---

kl!ck — небольшое окно с одной большой кнопкой. Вставляете ссылку на
подписку — получаете список серверов, остаток трафика и срок действия;
нажимаете кнопку — компьютер идёт через VPN. Можно пустить через VPN всё,
а можно только выбранное: заблокированные в России сайты, свои программы и
сайты.

VPN держит служба Windows, а не окно: окно работает без прав администратора,
его можно закрыть — VPN и Kill Switch продолжат работать. Под капотом —
**[mihomo](https://github.com/MetaCubeX/mihomo)** от MetaCubeX, официальный
релиз без изменений.

## Скриншоты

<table>
  <tr>
    <td align="center"><img src="docs/screenshots/home-dark.png" width="260" /><br/><sub>Главная</sub></td>
    <td align="center"><img src="docs/screenshots/home-light.png" width="260" /><br/><sub>Главная · светлая тема</sub></td>
    <td align="center"><img src="docs/screenshots/connection-dark.png" width="260" /><br/><sub>Соединение: режим и маршрутизация</sub></td>
  </tr>
  <tr>
    <td align="center"><img src="docs/screenshots/tray-dark.png" width="220" /><br/><sub>Окно трея — по правому щелчку</sub></td>
    <td align="center"><img src="docs/screenshots/killswitch-light.png" width="260" /><br/><sub>Kill Switch</sub></td>
    <td align="center"><img src="docs/screenshots/add-light.png" width="260" /><br/><sub>Добавить подключение</sub></td>
  </tr>
</table>

## Что умеет

**Подключения**
- **Подписки** по `https://` — Remnawave, Marzban, 3x-ui и совместимые; ответ
  в формате Clash/mihomo (YAML или JSON), sing-box, Xray или списком ссылок.
  Ключи Outline `ssconf://`. Остаток
  трафика и срок действия — из заголовка `subscription-userinfo`; подписки
  обновляются сами (как просит панель или раз в 12 часов), выбранный сервер
  сохраняется.
- **Одиночные ссылки** `vless://` (TLS, Reality), `vmess://`, `trojan://`,
  `ss://`, `hysteria2://` / `hy2://`, `tuic://`, `anytls://`, `wireguard://`,
  `socks5://`. Несколько строк сразу — одно подключение со списком серверов.
- **Файлы**: конфиги mihomo/Clash (YAML и JSON), sing-box и Xray/V2Ray
  (`.json`, в том числе массивы Remnawave), WireGuard и AmneziaWG (`.conf`),
  Shadowsocks SIP008, списки ссылок. Из чужих форматов берутся серверы.
- Кнопка «Добавить в kl!ck» на странице подписки (ссылка `klick://add`)
  открывает kl!ck с уже вставленной подпиской — остаётся нажать «Добавить».
- Предупреждения о конце срока и трафика подписки.

**Серверы**
- Переключение на лету, без переподключения.
- Задержка — настоящий запрос через сервер, а не пинг; меряется и при
  выключенном VPN.
- Если сервер перестал отвечать: переподключиться и ждать, перейти на
  следующий рабочий или всегда держаться самого быстрого — на выбор.
- После смены сети и пробуждения компьютера связь проверяется сразу.

**Режимы**
- **VPN (TUN)** — весь трафик всех программ, включая игры и UDP.
- **Системный прокси** — для браузеров и программ, которые понимают
  системный прокси. Прежние настройки прокси возвращаются как были, в том
  числе после выключения компьютера.
- Если рядом работает то, что мешает VPN (zapret, GoodbyeDPI, другой
  VPN-клиент), kl!ck предупредит.

**Куда направлять**
- **VPN для всего** — через VPN идёт всё; российские сайты и адреса можно
  пустить напрямую переключателями.
- **VPN для выбранного** — через VPN только нужное: готовый набор
  «Заблокированное в РФ» (встроенный список плюс общий список сообщества
  [itdoginfo/allow-domains](https://github.com/itdoginfo/allow-domains),
  обновляется раз в сутки) и ваш список. Остальное — напрямую.
- В свой список добавляются сервисы из каталога, программы, сайты и IP.
  Правило можно выключить, не удаляя.
- Локальная сеть — роутер, принтер, NAS — всегда напрямую.

**Соединение** — отдельная вкладка
- **Сейчас в сети:** какие программы куда ходят — через VPN или напрямую, —
  и тумблер у каждой, чтобы поменять.
- **Не открывается?** — неудачные соединения и причина.
- **Как вас видят сайты** — IP, страна и провайдер через VPN и напрямую,
  утечки DNS и IPv6.
- Живая схема: что сейчас идёт через VPN, что напрямую.

**Kill Switch**
- Отмеченные программы не выйдут в интернет мимо VPN: пока VPN выключен или
  оборвался, у них нет сети. Работает на уровне фильтров Windows (WFP),
  держится и без службы, и после перезагрузки.
- Локальная сеть для них открыта.
- Программа обновилась в новую папку или пропала — kl!ck заметит.

**Трей**
- Значок — клавиша цвета состояния: синяя — подключаюсь, зелёная —
  подключено, оранжевая — сервер не отвечает, красная — ошибка, серая —
  выключено.
- **Левый щелчок** — главное окно. **Правый** — окно трея: кнопка, скорость,
  «VPN для всего / для выбранного», подключения, серверы с задержкой,
  Kill Switch, выход.
- Уведомления Windows об обрывах и восстановлении связи.

**Прочее**
- Темы: системная, светлая, тёмная или своя — основы «Графит», «Полночь»,
  OLED, светлая и шесть цветов акцента.
- Запуск с Windows свёрнутым в трей; «Восстанавливать подключение» после
  перезагрузки.
- Журнал службы и отчёт для поддержки — без адресов сайтов и серверов.
- «О приложении»: версии, проверка обновлений на GitHub.

## Установка

1. Скачайте `klick-x.y.z-x64-setup.exe` со страницы [релизов](https://github.com/limeflash/klick/releases/latest).
2. Запустите — нужны Windows 10/11 x64 и один раз права администратора: они
   нужны, чтобы поставить службу. Если нет WebView2 Runtime, установщик
   поставит его сам.
3. «Добавить» → вставьте ссылку на подписку → кнопка питания.

Прежнюю kl!ck установщик найдёт сам, перенесёт подписки и удалит её.

Данные — в `C:\ProgramData\klick`: настройки, подписки (ссылки зашифрованы
DPAPI), журнал. Папка закрыта от обычных пользователей и программ.

**Приватность.** kl!ck ничего не собирает и никуда не отправляет. Сам он
ходит в сеть за вашими подписками, раз в сутки — за списком
«Заблокированное в РФ» (GitHub, через VPN), при установке — за WebView2
Runtime (Microsoft), если его нет; по кнопке — за проверкой IP (ipify.org,
proxycheck.io) и номером последней версии (GitHub API).

## macOS

kl!ck работает и на macOS 12+ (Apple Silicon и Intel) — то же окно, та же служба и те же функции;
Kill Switch — брандмауэром pf. Эта сборка живёт в [limeflash/klick](https://github.com/limeflash/klick):
в её [Releases](https://github.com/limeflash/klick/releases) — и установщик для Windows, и пакет для Mac;
обновления из [vbu00/klick](https://github.com/vbu00/klick) вливаются сюда. Пакет пока не подписан
сертификатом Apple, поэтому ставится из Терминала (`installer` ставит его без вопроса Gatekeeper):

```sh
curl -fL -o /tmp/klick.pkg https://github.com/limeflash/klick/releases/latest/download/klick-macos.pkg
sudo installer -pkg /tmp/klick.pkg -target /
open -a 'kl!ck'
```

Пакет ставит окно в «Программы» и службу (демон launchd `app.klick.service`); данные — в
`/Library/Application Support/klick`. Обновление — те же команды: служба перезапустится и, если VPN
был включён, включит его снова. Пути с `kl!ck` в zsh пишите в **одинарных** кавычках.

```sh
'/Applications/kl!ck.app/Contents/MacOS/klick-cli' --prod status      # состояние VPN
sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh'        # удалить (подключения и настройки остаются)
sudo '/Applications/kl!ck.app/Contents/Resources/uninstall.sh' --wipe # удалить всё
```

Как устроен порт и чем macOS отличается от Windows — в [docs/macos-port.md](docs/macos-port.md).

## iOS

kl!ck для iPhone и iPad (iOS 15+) — то же окно, подписки, правила и проверка серверов; VPN держит
системное расширение, поэтому он работает и с закрытым kl!ck. В App Store его нет: файл
[`klick-ios.ipa`](https://github.com/limeflash/klick/releases/latest/download/klick-ios.ipa) нужно
подписать самому, и для VPN в подписи нужны права Network Extension и App Groups — их даёт только
**платный** аккаунт разработчика Apple (свой сертификат через ESign/Feather, SideStore или Sideloadly).
С бесплатным Apple ID приложение встанет, но VPN не включится. Источник для AltStore, SideStore,
Feather и ESign: `https://github.com/limeflash/klick/releases/latest/download/klick-altstore.json`.

Как поставить — [docs/ios-install.md](https://github.com/limeflash/klick/blob/ios/docs/ios-install.md);
как устроен порт и чего на iOS нет (Kill Switch по программам, системный прокси) —
[docs/ios-port.md](https://github.com/limeflash/klick/blob/ios/docs/ios-port.md). Код iOS — в ветке
[`ios`](https://github.com/limeflash/klick/tree/ios).

## Сборка

Нужны Rust (stable, MSVC), Node.js 20.19+ и PowerShell.

```powershell
cd ui; npm install; cd ..
powershell -ExecutionPolicy Bypass -File build.ps1   # dist\klick-setup.exe
```

На Mac (Xcode Command Line Tools, Rust, Node.js 20.19+):

```sh
scripts/macos/build.sh             # dist/kl!ck.app и dist/klick-<версия>.pkg (universal)
scripts/ios/build.sh               # dist/klick-ios.ipa (ветка ios; ещё Xcode, Go, brew install xcodegen)
```

Ядро mihomo и база стран в git не хранятся: `build.ps1` скачивает
официальный релиз и проверяет sha256 (`tools\fetch-core.ps1`), на Mac — `scripts/macos/fetch-core.sh`.
Как устроены исходники, служба для разработки, превью окна и проверка в
Песочнице Windows — в [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md).
Скриншоты для README снимает `npm run screenshots`, видеоинструкцию — `npm run video`
(GIF — `npm run video:gif`) в папке `ui`.

## Авторы

- [**vbu00**](https://github.com/vbu00) — разработка
- [**Dmitriy Medvedev**](https://github.com/aleuuu) — дизайн интерфейса и логотип
- [**limeflash**](https://github.com/limeflash) — порт на macOS и iOS

Логотип-клавиша в пяти цветах и вордмарк — в [`docs/logo`](docs/logo).

## Лицензия

kl!ck — [MIT](LICENSE). mihomo — GPL-3.0, поставляется отдельным файлом
без изменений. На iOS отдельных программ не бывает: там mihomo собран в приложение библиотекой, поэтому
`klick-ios.ipa` целиком распространяется на условиях GPL-3.0 (исходники — этот репозиторий). Подробности — [THIRD_PARTY_NOTICES.md](resources/licenses/THIRD_PARTY_NOTICES.md).

Спасибо [MetaCubeX](https://github.com/MetaCubeX) за mihomo.
