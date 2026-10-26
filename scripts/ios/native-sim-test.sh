#!/bin/bash
# kl!ck β в симуляторе iOS: движок запускается, полная самопроверка (каждая команда движка, которой
# пользуются экраны, — Sources/SelfTest.swift), приложение не падает, снимки вкладок и подэкранов в светлой
# и тёмной теме — dist/ios-beta-*.png, итог самопроверки — dist/ios-beta-selftest.log. С KLICK_TEST_SUB
# приложение само добавляет подписку и меряет задержку (Engine.selfTest), и на снимках видны серверы.
#   scripts/ios/native-sim-test.sh dist/klick-ios-beta-sim.app
set -euo pipefail
app="${1:?путь к .app для симулятора}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
bundle_id="app.klick.native"

# iPhone на самой новой iOS: на iOS 26 видно Liquid Glass.
device="$(xcrun simctl list devices available -j | jq -r '[.devices | to_entries[] | select(.key | test("iOS"))
    | (.key | capture("iOS-(?<a>[0-9]+)-(?<b>[0-9]+)") | [(.a | tonumber), (.b | tonumber)]) as $v
    | .value[] | select(.name | test("^iPhone")) | {udid, v: $v}] | sort_by(.v) | last | .udid')"
[[ -n "$device" && "$device" != null ]] || { echo "нет симулятора iPhone" >&2; exit 1; }
echo "симулятор: $(xcrun simctl list devices -j | jq -r --arg d "$device" '.devices | to_entries[] | .key as $k | .value[] | select(.udid == $d) | "\(.name) · \($k | sub(".*SimRuntime."; ""))"')"
xcrun simctl boot "$device" 2>/dev/null || true
xcrun simctl bootstatus "$device" -b >/dev/null
xcrun simctl install "$device" "$app"

# launch <вкладка> <light|dark> <0|1 — стекло> [ИМЯ=значение…]: 0 главная, 1 соединение, 2 добавить,
# 3 настройки; дальше — что открыть (KLICK_SETTINGS_SUB, KLICK_CONN_SUB, KLICK_EXPAND, KLICK_SELFTEST_FULL).
launch() {
    local tab="$1" theme="$2" glass="$3"
    shift 3
    xcrun simctl terminate "$device" "$bundle_id" 2>/dev/null || true
    xcrun simctl ui "$device" appearance "$theme"
    env SIMCTL_CHILD_KLICK_SELFTEST_SUB="${KLICK_TEST_SUB:-}" SIMCTL_CHILD_KLICK_TAB="$tab" SIMCTL_CHILD_KLICK_THEME="$theme" \
        SIMCTL_CHILD_KLICK_GLASS="$glass" ${@+"${@/#/SIMCTL_CHILD_}"} xcrun simctl launch "$device" "$bundle_id" >/dev/null
}
# Список целиком, а не `| grep -q`: grep выходит на первом совпадении, launchctl получает SIGPIPE, и с
# pipefail живое приложение выглядело закрывшимся.
alive() { local list; list="$(xcrun simctl spawn "$device" launchctl list)" && [[ "$list" == *"$bundle_id"* ]]; }
# Отчёт о падении (симулятор пишет их в ~/Library/Logs/DiagnosticReports хоста): причина и верх
# упавшего потока — в журнал CI, сам отчёт — в dist.
crash() { # crash <метка>
    local ips
    ips="$(ls -t "$HOME"/Library/Logs/DiagnosticReports/KlickNative*.ips 2>/dev/null | head -1 || true)"
    [[ -n "$ips" ]] || { echo "     отчёта о падении нет"; return 0; }
    cp "$ips" "$root/dist/ios-beta-crash-$1.ips"
    tail -n +2 "$ips" | jq -r '
        "     \(.exception.type // "?") \(.exception.signal // "") \(.exception.subtype // "")",
        (.asi // {} | to_entries[] | "     \(.key): \(.value | join(" "))"),
        (.usedImages as $img | .threads[] | select(.triggered == true) | .frames[:25][]
            | "     \($img[.imageIndex].name // "?") \(.symbol // "0x\(.imageOffset)")")' 2>/dev/null | head -40 || true
    rm -f "$ips"
    xcrun simctl spawn "$device" log show --last 2m --style compact --predicate 'process == "KlickNative" AND messageType >= 16' 2>/dev/null | tail -15 || true
}
shot() { xcrun simctl io "$device" screenshot "$root/dist/ios-beta-$1.png" >/dev/null 2>&1 && echo "снимок: dist/ios-beta-$1.png"; }
# view <снимок> <вкладка> <тема> <стекло> [ИМЯ=значение…]: открыть, подождать, снять, проверить, что живо.
view() {
    local name="$1"
    shift
    launch "$@"
    sleep 6
    shot "$name"
    alive || { echo "FAIL $name: приложение закрылось"; crash "$name"; bad=1; }
}

mkdir -p "$root/dist"
rm -f "$HOME"/Library/Logs/DiagnosticReports/KlickNative*.ips
bad=0

# Первый запуск: подписка, задержка и полная самопроверка.
launch 0 light 1 KLICK_SELFTEST_FULL=1
data="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
log="$data/Library/Application Support/klick/logs/service.log"
selftest="$data/Library/Application Support/klick/logs/selftest.log"
for ((i = 0; i < 60; i++)); do grep -q "данные:" "$log" 2>/dev/null && break; sleep 1; done
if grep -q "данные:" "$log" 2>/dev/null; then echo "ok   движок запустился"; else echo "FAIL движок не запустился за 60 с"; bad=1; fi
if [[ -n "${KLICK_TEST_SUB:-}" ]]; then
    for ((i = 0; i < 180; i++)); do grep -q "selftest done" "$selftest" 2>/dev/null && break; sleep 1; done
    if grep -q "selftest done" "$selftest" 2>/dev/null; then
        grep -v "selftest done" "$selftest" | sed 's/^/     /'
        grep -q "^FAIL" "$selftest" && { echo "FAIL самопроверка: есть ошибки"; bad=1; } || echo "ok   самопроверка"
    else
        echo "FAIL самопроверка не закончилась за 3 минуты"
        bad=1
    fi
    cp "$selftest" "$root/dist/ios-beta-selftest.log" 2>/dev/null || true
else
    sleep 6
fi
shot home-light
if alive; then echo "ok   приложение работает"; else echo "FAIL приложение закрылось"; crash home; bad=1; fi

view tab1-light 1 light 1
view tab2-light 2 light 1
view tab3-light 3 light 1
view tab0-dark 0 dark 1
view tab3-dark 3 dark 0
# Подэкраны: серверы в карточке, «Тема», «О приложении», мой список, лист «Добавить», «Сейчас в сети».
view servers-dark 0 dark 1 KLICK_EXPAND=1
view theme-dark 3 dark 1 KLICK_SETTINGS_SUB=theme
view about-light 3 light 0 KLICK_SETTINGS_SUB=about
view list-light 1 light 1 KLICK_CONN_SUB=list
view addrule-dark 1 dark 1 KLICK_CONN_SUB=add
view live-light 1 light 0 KLICK_CONN_SUB=live

xcrun simctl ui "$device" appearance light
grep -E "ERROR|паника|panicked" "$log" 2>/dev/null | tail -5 || true
cp "$log" "$root/dist/ios-beta-service.log" 2>/dev/null || true
exit $bad
