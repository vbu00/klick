#!/bin/bash
# Свои серверы для проверки kl!ck в симуляторе: Hysteria2 и VLESS Reality на 127.0.0.2 (сервером
# работает mihomo для Mac) и подписка с ними — http://127.0.0.1:24480/sub. Не 127.0.0.1: сервер на нём
# kl!ck считает заглушкой панели («лимит устройств»), а такую подписку — отказом. Симулятор живёт в сети Mac,
# поэтому kl!ck в нём подключается к этим серверам по-настоящему: рукопожатие QUIC и Reality, а через
# сервер — в интернет. Печатает ссылку подписки; серверы работают, пока не завершится CI. Вывод — в файл,
# а не в $(…): подстановка ждала бы, пока закроют вывод все процессы, а серверы работают дальше.
#   scripts/ios/test-servers.sh >sub.txt && KLICK_TEST_SUB="$(cat sub.txt)" scripts/ios/sim-test.sh …
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
core="$root/resources/core/mihomo"
[[ -x "$core" ]] || "$root/scripts/macos/fetch-core.sh" >&2
work="$(mktemp -d)"
host=127.0.0.2
ifconfig lo0 | grep -q "inet $host " || sudo ifconfig lo0 alias "$host" up

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
    listen: $host
    users: { ci: "$password" }
    certificate: $work/cert.pem
    private-key: $work/key.pem
  - name: vless
    type: vless
    port: 24444
    listen: $host
    users: [{ username: ci, uuid: "$uuid", flow: xtls-rprx-vision }]
    reality-config:
      dest: www.apple.com:443
      private-key: "$private"
      short-id: ["$short_id"]
      server-names: [www.apple.com]
mode: rule
rules: ["MATCH,DIRECT"]
YAML
nohup "$core" -d "$work" -f "$work/server.yaml" </dev/null >"$work/server.log" 2>&1 &

printf '%s\n' \
    "hysteria2://$password@$host:24443?insecure=1&sni=localhost#CI%20Hysteria2" \
    "vless://$uuid@$host:24444?encryption=none&security=reality&pbk=$public&sid=$short_id&sni=www.apple.com&fp=chrome&type=tcp&flow=xtls-rprx-vision#CI%20VLESS" \
    >"$work/sub"
(cd "$work" && nohup python3 -m http.server 24480 --bind 127.0.0.1 </dev/null >/dev/null 2>&1 &)

for _ in $(seq 1 50); do
    if nc -z -G 1 "$host" 24444 2>/dev/null && nc -z -G 1 127.0.0.1 24480 2>/dev/null; then
        echo "http://127.0.0.1:24480/sub"
        exit 0
    fi
    sleep 0.2
done
echo "тестовые серверы не запустились:" >&2
cat "$work/server.log" >&2
exit 1
