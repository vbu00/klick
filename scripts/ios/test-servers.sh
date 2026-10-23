#!/bin/bash
# Свои серверы для проверки kl!ck в симуляторе: Hysteria2 и VLESS Reality на 127.0.0.1 (сервером
# работает mihomo для Mac) и подписка с ними — http://127.0.0.1:24480/sub. Симулятор живёт в сети Mac,
# поэтому kl!ck в нём подключается к этим серверам по-настоящему: рукопожатие QUIC и Reality, а через
# сервер — в интернет. Печатает ссылку подписки; серверы работают, пока не завершится CI.
#   KLICK_TEST_SUB="$(scripts/ios/test-servers.sh)" scripts/ios/sim-test.sh dist/klick-ios-sim.app
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
core="$root/resources/core/mihomo"
[[ -x "$core" ]] || "$root/scripts/macos/fetch-core.sh" >&2
work="$(mktemp -d)"

openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:prime256v1 -nodes -days 2 -subj /CN=localhost \
    -keyout "$work/key.pem" -out "$work/cert.pem" >/dev/null 2>&1
keys="$("$core" generate reality-keypair)"
private="$(sed -n 's/^PrivateKey: //p' <<<"$keys")"
public="$(sed -n 's/^PublicKey: //p' <<<"$keys")"
uuid="$(uuidgen | tr '[:upper:]' '[:lower:]')"
password="$(openssl rand -hex 12)"
short_id="$(openssl rand -hex 4)"

cat >"$work/server.yaml" <<YAML
mixed-port: 0
log-level: warning
dns:
  enable: true
  ipv6: false
  nameserver: [8.8.8.8, https://1.1.1.1/dns-query]
listeners:
  - name: hy2
    type: hysteria2
    port: 24443
    listen: 127.0.0.1
    users: { ci: "$password" }
    certificate: $work/cert.pem
    private-key: $work/key.pem
  - name: vless
    type: vless
    port: 24444
    listen: 127.0.0.1
    users: [{ username: ci, uuid: "$uuid", flow: xtls-rprx-vision }]
    reality-config:
      dest: www.apple.com:443
      private-key: "$private"
      short-id: ["$short_id"]
      server-names: [www.apple.com]
mode: rule
rules: ["MATCH,DIRECT"]
YAML
nohup "$core" -d "$work" -f "$work/server.yaml" >"$work/server.log" 2>&1 &

printf '%s\n' \
    "hysteria2://$password@127.0.0.1:24443?insecure=1&sni=localhost#CI%20Hysteria2" \
    "vless://$uuid@127.0.0.1:24444?encryption=none&security=reality&pbk=$public&sid=$short_id&sni=www.apple.com&fp=chrome&type=tcp&flow=xtls-rprx-vision#CI%20VLESS" \
    >"$work/sub"
(cd "$work" && nohup python3 -m http.server 24480 --bind 127.0.0.1 >/dev/null 2>&1 &)

for _ in $(seq 1 50); do
    if nc -z 127.0.0.1 24444 && nc -z 127.0.0.1 24480; then
        echo "http://127.0.0.1:24480/sub"
        exit 0
    fi
    sleep 0.2
done
echo "тестовые серверы не запустились:" >&2
cat "$work/server.log" >&2
exit 1
