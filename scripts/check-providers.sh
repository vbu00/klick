#!/usr/bin/env bash
# Starts mihomo for each real provider file. `mihomo -t` alone does not load file providers.
set -uo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -ne 1 ]]; then
    echo "Использование: $0 <путь-к-mihomo>" >&2
    exit 2
fi

core_arg="$1"
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        command -v cygpath >/dev/null 2>&1 && core_arg="$(cygpath -u "$core_arg")"
        ;;
esac
if [[ "$core_arg" != /* && ! "$core_arg" =~ ^[A-Za-z]:[\\/] ]]; then
    core_arg="$root/$core_arg"
fi
[[ -x "$core_arg" ]] || { echo "Не найден исполняемый mihomo: $core_arg" >&2; exit 2; }

is_windows=0
ps=""
case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*)
        is_windows=1
        ps="powershell.exe"
        ;;
esac

json_override() {
    if command -v jq >/dev/null 2>&1; then
        printf '%s\n' "$1" | jq -ce '."proxy-providers"."klick-servers".override'
        return
    fi
    [[ $is_windows -eq 1 ]] || { echo "Нужен jq для чтения dump_config" >&2; return 1; }
    local command_text
    command_text='$d=[Console]::In.ReadToEnd() | ConvertFrom-Json; $d."proxy-providers"."klick-servers".override | ConvertTo-Json -Depth 20 -Compress'
    printf '%s\n' "$1" | "$ps" -NoLogo -NoProfile -NonInteractive -Command "$command_text"
}

write_config() {
    local config_path="$1" port="$2" override="$3"
    if command -v jq >/dev/null 2>&1; then
        jq -n --arg port "$port" --argjson override "$override" '{
          mode: "rule",
          "log-level": "debug",
          "external-controller": ("127.0.0.1:" + $port),
          "proxy-providers": {
            "klick-servers": {
              type: "file",
              path: "provider.yaml",
              interval: 86400,
              override: $override
            }
          },
          "proxy-groups": [{name: "PROVIDER-CHECK", type: "select", use: ["klick-servers"]}],
          rules: ["MATCH,PROVIDER-CHECK"]
        }' > "$config_path"
        return
    fi
    [[ $is_windows -eq 1 ]] || { echo "Нужен jq для сборки конфига" >&2; return 1; }
    local native_config command_text
    native_config="$(cygpath -w "$config_path")"
    command_text='$override=ConvertFrom-Json -InputObject $env:KLICK_PROVIDER_OVERRIDE; $cfg=[ordered]@{ mode="rule"; "log-level"="debug"; "external-controller"=("127.0.0.1:"+$env:KLICK_PROVIDER_PORT); "proxy-providers"=[ordered]@{ "klick-servers"=[ordered]@{ type="file"; path="provider.yaml"; interval=86400; override=$override } }; "proxy-groups"=@([ordered]@{ name="PROVIDER-CHECK"; type="select"; use=@("klick-servers") }); rules=@("MATCH,PROVIDER-CHECK") }; [IO.File]::WriteAllText($env:KLICK_CONFIG,($cfg | ConvertTo-Json -Depth 32),(New-Object Text.UTF8Encoding $false))'
    KLICK_PROVIDER_OVERRIDE="$override" KLICK_PROVIDER_PORT="$port" KLICK_CONFIG="$native_config" \
        "$ps" -NoLogo -NoProfile -NonInteractive -Command "$command_text"
}

json_proxy_count() {
    if command -v jq >/dev/null 2>&1; then
        printf '%s\n' "$1" | jq -er '.proxies | length'
        return
    fi
    [[ $is_windows -eq 1 ]] || { echo "Нужен jq для ответа mihomo" >&2; return 1; }
    local command_text
    command_text='$r=[Console]::In.ReadToEnd() | ConvertFrom-Json; if ($null -eq $r.proxies) { exit 3 }; Write-Output (@($r.proxies).Count)'
    printf '%s\n' "$1" | "$ps" -NoLogo -NoProfile -NonInteractive -Command "$command_text"
}

free_port() {
    local port=""
    if command -v python3 >/dev/null 2>&1; then
        port="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || true)"
    fi
    if [[ ! "$port" =~ ^[0-9]+$ ]] && command -v python >/dev/null 2>&1; then
        port="$(python -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()' 2>/dev/null || true)"
    fi
    if [[ ! "$port" =~ ^[0-9]+$ && $is_windows -eq 1 ]]; then
        port="$("$ps" -NoLogo -NoProfile -NonInteractive -Command '$l=[Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0); $l.Start(); $p=$l.LocalEndpoint.Port; $l.Stop(); Write-Output $p' 2>/dev/null | tr -d '\r')"
    fi
    [[ "$port" =~ ^[0-9]+$ ]] || { echo "Не удалось выбрать свободный порт для external-controller" >&2; return 1; }
    printf '%s\n' "$port"
}

if command -v jq >/dev/null 2>&1; then
    :
elif [[ $is_windows -eq 1 ]] && command -v powershell.exe >/dev/null 2>&1; then
    :
else
    echo "Нужен jq (или Windows PowerShell в Git Bash) для разбора JSON" >&2
    exit 2
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "Не найден cargo: нужен examples/dump_config для точного override" >&2
    exit 2
fi
dump_json="$(cd "$root" && cargo run -q -p klick-core --example dump_config -- proxy all_vpn windows)" || {
    echo "Не удалось получить конфиг из klick-core/examples/dump_config.rs" >&2
    exit 1
}
provider_override="$(json_override "$dump_json")" || {
    echo "Не удалось прочитать proxy-providers.klick-servers.override из dump_config" >&2
    exit 1
}

work="$(mktemp -d)" || exit 1
work_real="$(cd "$work" && pwd -P)" || exit 1
temp_parent_real="$(cd "$(dirname "$work")" && pwd -P)" || exit 1
if [[ "$work_real" == "$temp_parent_real" || "$(dirname "$work_real")" != "$temp_parent_real" ]]; then
    echo "FAIL: mktemp returned a directory outside its temporary parent: $work_real" >&2
    exit 1
fi
core_pid=""
current_log=""
cleanup() {
    if [[ -n "$core_pid" ]]; then
        kill "$core_pid" 2>/dev/null || true
        wait "$core_pid" 2>/dev/null || true
    fi
    if [[ -n "$work_real" && -n "$temp_parent_real" && "$work_real" != "$temp_parent_real" && "$(dirname "$work_real")" == "$temp_parent_real" ]]; then
        rm -rf -- "$work_real"
    fi
}
trap cleanup EXIT INT TERM

show_core_error() {
    local log="$1"
    echo "--- строки ошибок mihomo ($log)"
    if ! grep -Ei 'error|failed|invalid|provider' "$log" 2>/dev/null | tail -30; then
        tail -30 "$log" 2>/dev/null || true
    fi
}

cases=(
    "active_klick|$root/crates/klick-core/tests/fixtures/mistgate/active_klick.body|7"
    "unlimited_klick|$root/crates/klick-core/tests/fixtures/mistgate/unlimited_klick.body|7"
    "unicode_title_klick|$root/crates/klick-core/tests/fixtures/mistgate/unicode_title_klick.body|7"
    "golden_all_variants|$root/crates/klick-core/tests/fixtures/mistgate/golden_all_variants.body|16"
    "active_curl|$root/crates/klick-core/tests/fixtures/mistgate/active_curl.body|4"
    "active_curl_converted|$root/scripts/provider-fixtures/active_curl-converted.yaml|4"
    "active_singbox_converted|$root/scripts/provider-fixtures/active_singbox-converted.yaml|3"
    "active_klick_xrayjson_converted|$root/scripts/provider-fixtures/active_klick_xrayjson-converted.yaml|4"
)

failed=0
for entry in "${cases[@]}"; do
    IFS='|' read -r name fixture expected <<< "$entry"
    case_home="$work/$name"
    mkdir -p "$case_home"
    if [[ ! -f "$fixture" ]]; then
        echo "FAIL $name: нет файла провайдера $fixture"
        failed=1
        continue
    fi
    cp "$fixture" "$case_home/provider.yaml"
    port="$(free_port)" || { failed=1; continue; }
    config="$case_home/config.yaml"
    log="$case_home/mihomo.log"
    if ! write_config "$config" "$port" "$provider_override"; then
        echo "FAIL $name: не удалось создать минимальный конфиг mihomo"
        failed=1
        continue
    fi

    "$core_arg" -d "$case_home" -f "$config" > "$log" 2>&1 &
    core_pid=$!
    response=""
    count=""
    ready=0
    for _ in $(seq 1 120); do
        if ! kill -0 "$core_pid" 2>/dev/null; then
            wait "$core_pid" 2>/dev/null || true
            core_pid=""
            break
        fi
        response="$(curl -fsS --connect-timeout 1 --max-time 3 "http://127.0.0.1:$port/providers/proxies/klick-servers" 2>/dev/null || true)"
        if [[ "$response" == *'"proxies"'* ]]; then
            count="$(json_proxy_count "$response" 2>/dev/null || true)"
            if [[ "$count" == "$expected" ]]; then
                ready=1
                break
            fi
        fi
        sleep 0.25
    done

    if [[ $ready -ne 1 ]]; then
        if [[ "$response" == *'"proxies"'* ]]; then
            echo "FAIL $name: за 30 с загружено ${count:-неизвестно} серверов, ожидалось $expected"
        else
            echo "FAIL $name: mihomo не отдал /providers/proxies/klick-servers за 30 с"
        fi
        show_core_error "$log"
        failed=1
    else
        echo "OK   $name: $count серверов"
    fi

    kill "$core_pid" 2>/dev/null || true
    wait "$core_pid" 2>/dev/null || true
    core_pid=""
done

if [[ $failed -ne 0 ]]; then
    echo "Проверка proxy-provider завершилась с ошибками"
    exit 1
fi
echo "Все файлы proxy-provider загружены"
