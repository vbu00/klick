#!/bin/bash
# kl!ck β в симуляторе iOS: движок запускается, приложение не падает, снимки трёх вкладок в светлой и
# тёмной теме — dist/ios-beta-*.png. С KLICK_TEST_SUB приложение само добавляет подписку и меряет задержку
# (Engine.selfTest), и на снимке главной видны серверы.
#   scripts/ios/native-sim-test.sh dist/klick-ios-beta-sim.app
set -euo pipefail
app="${1:?путь к .app для симулятора}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
bundle_id="app.klick.native"

device="$(xcrun simctl list devices available -j | jq -r '[.devices | to_entries[] | select(.key | test("iOS")) | .value[] | select(.name | test("^iPhone"))] | last | .udid')"
[[ -n "$device" && "$device" != null ]] || { echo "нет симулятора iPhone" >&2; exit 1; }
echo "симулятор: $(xcrun simctl list devices -j | jq -r --arg d "$device" '.devices[][] | select(.udid == $d) | .name')"
xcrun simctl boot "$device" 2>/dev/null || true
xcrun simctl bootstatus "$device" -b >/dev/null
xcrun simctl install "$device" "$app"

launch() { # launch <вкладка>
    xcrun simctl terminate "$device" "$bundle_id" 2>/dev/null || true
    SIMCTL_CHILD_KLICK_SELFTEST_SUB="${KLICK_TEST_SUB:-}" SIMCTL_CHILD_KLICK_TAB="$1" xcrun simctl launch "$device" "$bundle_id" >/dev/null
}
alive() { xcrun simctl spawn "$device" launchctl list | grep -q "$bundle_id"; }
shot() { xcrun simctl io "$device" screenshot "$root/dist/ios-beta-$1.png" >/dev/null 2>&1 && echo "снимок: dist/ios-beta-$1.png"; }

mkdir -p "$root/dist"
bad=0
xcrun simctl ui "$device" appearance light
launch 0
data="$(xcrun simctl get_app_container "$device" "$bundle_id" data)"
log="$data/Library/Application Support/klick/logs/service.log"
for ((i = 0; i < 60; i++)); do grep -q "данные:" "$log" 2>/dev/null && break; sleep 1; done
if grep -q "данные:" "$log" 2>/dev/null; then echo "ok   движок запустился"; else echo "FAIL движок не запустился за 60 с"; bad=1; fi
# Подписка, список серверов и задержка через каждый — до полуминуты.
if [[ -n "${KLICK_TEST_SUB:-}" ]]; then sleep 30; else sleep 6; fi
shot home-light
if alive; then echo "ok   приложение работает"; else echo "FAIL приложение закрылось"; bad=1; fi
for tab in 1 2; do
    launch "$tab"
    sleep 6
    shot "tab$tab-light"
    alive || { echo "FAIL вкладка $tab: приложение закрылось"; bad=1; }
done
xcrun simctl ui "$device" appearance dark
for tab in 0 2; do
    launch "$tab"
    sleep 6
    shot "tab$tab-dark"
    alive || { echo "FAIL тёмная тема, вкладка $tab: приложение закрылось"; bad=1; }
done
xcrun simctl ui "$device" appearance light
grep -E "ERROR|паника|panicked" "$log" 2>/dev/null | tail -5 || true
cp "$log" "$root/dist/ios-beta-service.log" 2>/dev/null || true
exit $bad
