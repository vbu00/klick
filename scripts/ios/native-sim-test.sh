#!/bin/bash
# kl!ck β в симуляторе iOS: движок запускается, приложение не падает, снимки трёх вкладок в светлой и
# тёмной теме — dist/ios-beta-*.png. С KLICK_TEST_SUB приложение само добавляет подписку и меряет задержку
# (Engine.selfTest), и на снимке главной видны серверы.
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

launch() { # launch <вкладка> [light|dark] [1 — стекло]: 0 главная, 1 соединение, 2 добавить, 3 настройки
    xcrun simctl terminate "$device" "$bundle_id" 2>/dev/null || true
    SIMCTL_CHILD_KLICK_SELFTEST_SUB="${KLICK_TEST_SUB:-}" SIMCTL_CHILD_KLICK_TAB="$1" SIMCTL_CHILD_KLICK_THEME="${2:-}" SIMCTL_CHILD_KLICK_GLASS="${3:-0}"         xcrun simctl launch "$device" "$bundle_id" >/dev/null
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

mkdir -p "$root/dist"
rm -f "$HOME"/Library/Logs/DiagnosticReports/KlickNative*.ips
bad=0
xcrun simctl ui "$device" appearance light
launch 0 light 1
data="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
log="$data/Library/Application Support/klick/logs/service.log"
for ((i = 0; i < 60; i++)); do grep -q "данные:" "$log" 2>/dev/null && break; sleep 1; done
if grep -q "данные:" "$log" 2>/dev/null; then echo "ok   движок запустился"; else echo "FAIL движок не запустился за 60 с"; bad=1; fi
# Подписка, список серверов и задержка через каждый — до полуминуты.
if [[ -n "${KLICK_TEST_SUB:-}" ]]; then sleep 30; else sleep 6; fi
shot home-light
if alive; then echo "ok   приложение работает"; else echo "FAIL приложение закрылось"; crash home; bad=1; fi
for tab in 1 2 3; do
    launch "$tab" light 1
    sleep 6
    shot "tab$tab-light"
    alive || { echo "FAIL вкладка $tab: приложение закрылось"; crash "tab$tab-light"; bad=1; }
done
xcrun simctl ui "$device" appearance dark
for tab in 0 3; do
    launch "$tab" dark "$((tab == 0))"
    sleep 6
    shot "tab$tab-dark"
    alive || { echo "FAIL тёмная тема, вкладка $tab: приложение закрылось"; crash "tab$tab-dark"; bad=1; }
done
xcrun simctl ui "$device" appearance light
grep -E "ERROR|паника|panicked" "$log" 2>/dev/null | tail -5 || true
cp "$log" "$root/dist/ios-beta-service.log" 2>/dev/null || true
exit $bad
