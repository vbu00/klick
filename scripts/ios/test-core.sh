#!/bin/bash
# Юнит-тесты общей логики kl!ck под iOS (aarch64-apple-ios-sim) — прямо в симуляторе: klick-core
# (конфиг ядра, подписки, страж), klick-proto и движок службы klick-service.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
rustup target add aarch64-apple-ios-sim >/dev/null 2>&1 || true

device="$(xcrun simctl list devices available -j | jq -r '[.devices | to_entries[] | select(.key | test("iOS")) | .value[] | select(.name | test("^iPhone"))] | last | .udid')"
[[ -n "$device" && "$device" != null ]] || { echo "нет симулятора iPhone" >&2; exit 1; }
xcrun simctl boot "$device" 2>/dev/null || true
xcrun simctl bootstatus "$device" -b >/dev/null

exes="$(cd "$root" && cargo test -p klick-core -p klick-proto -p klick-service --lib --target aarch64-apple-ios-sim --no-run --message-format=json \
    | jq -r 'select(.reason == "compiler-artifact" and .profile.test == true and .executable != null) | .executable')"
bad=0
for exe in $exes; do
    echo "== $(basename "$exe")"
    xcrun simctl spawn "$device" "$exe" || bad=1
done
exit $bad
