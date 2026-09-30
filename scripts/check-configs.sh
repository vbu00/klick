#!/bin/bash
# Проверяет ядром (`mihomo -t`) все варианты конфига, которые собирает klick-core:
# режимы (TUN, прокси, проверочное ядро, страж Kill Switch), оба положения тумблера, Windows и macOS.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
core="$root/resources/core/mihomo"
[[ -x "$core" ]] || "$root/scripts/macos/fetch-core.sh"

home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
mkdir -p "$home/sets" "$home/providers"
cp "$root"/resources/sets/* "$home/sets/"
cp "$root/resources/core/Country.mmdb" "$home/"
echo 'vless://11111111-2222-3333-4444-555555555555@example.com:443?security=tls&type=tcp#test' > "$home/providers/test.txt"

(cd "$root" && cargo build -q -p klick-core --example dump_config)
dump="$root/target/debug/examples/dump_config"
bad=0
for os in macos windows; do
    for mode in tun proxy tester guard; do
        for routing in selected all_vpn; do
            cfg="$home/$os-$mode-$routing.yaml"
            "$dump" "$mode" "$routing" "$os" > "$cfg"
            if "$core" -d "$home" -f "$cfg" -t > "$home/out.txt" 2>&1; then
                echo "ok   $os $mode $routing"
            else
                echo "FAIL $os $mode $routing"; cat "$home/out.txt"; bad=1
            fi
        done
    done
done
exit $bad
