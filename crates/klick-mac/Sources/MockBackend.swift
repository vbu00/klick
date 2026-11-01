import Foundation

/// Тестовые данные вместо службы (`KLICK_MOCK=1`): снимки окна в CI без установленной kl!ck.
/// `KLICK_MOCK_STATE` — `connected` (по умолчанию), `off`, `reconnecting`, `error`.
final class MockBackend: Backend, @unchecked Sendable {
    var onEvent: ((Data) -> Void)?
    var onLink: ((Bool) -> Void)?

    private var vpn = ProcessInfo.processInfo.environment["KLICK_MOCK_STATE"] ?? "connected"
    private var server = "Германия · Frankfurt"
    private var routing = "all_vpn"
    private let since = Date().timeIntervalSince1970 - 5051

    func start() {
        onLink?(true)
        Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            guard let self, self.vpn == "connected" else { return }
            let down = Double.random(in: 3_800_000...6_200_000), up = Double.random(in: 180_000...420_000)
            self.emit(["t": "event", "ev": "traffic", "up": up, "down": down])
        }
    }

    func call(_ cmd: String, _ args: [String: Any]?) async -> ServiceLink.Reply {
        switch cmd {
        case "status": return ok(status())
        case "settings": return ok(settings())
        case "servers", "connection_servers", "test_latency": return ok(servers())
        case "catalog":
            return ok(["services": [
                ["id": "youtube", "name": "YouTube", "domains": ["youtube.com"], "cidrs": []],
                ["id": "telegram", "name": "Telegram", "domains": ["telegram.org"], "cidrs": []],
                ["id": "discord", "name": "Discord", "domains": ["discord.com"], "cidrs": []],
            ]])
        case "connections":
            return ok([
                ["host": "www.youtube.com", "process": "Google Chrome", "route": "vpn", "upload": 120_000, "download": 42_000_000],
                ["host": "yandex.ru", "process": "Google Chrome", "route": "direct", "upload": 9_000, "download": 310_000],
                ["host": "api.steampowered.com", "process": "Steam", "route": "direct", "upload": 40_000, "download": 30_600_000],
                ["host": "149.154.167.50", "process": "Telegram", "route": "vpn", "upload": 600_000, "download": 14_900_000],
            ])
        case "failures": return ok([["host": "rutracker.org", "route": "direct", "error": "timeout", "at": Date().timeIntervalSince1970 - 120]])
        case "about": return ok(["version": "0.5.4", "core_version": "v1.19.33", "os": "macOS 26.0"])
        case "log": return ok([["at": Date().timeIntervalSince1970 * 1000, "level": "info", "text": "VPN подключён · Германия · Frankfurt"]])
        case "speed_test": return ok(["server": server, "down": 84.7, "up": 43.2])
        case "ip_check":
            return ok(["via_vpn": ["ipv4": "185.23.11.4", "country": "Германия", "country_code": "DE", "city": "Франкфурт", "provider": "Hetzner"],
                       "direct": ["ipv4": "95.26.14.2", "country": "Россия", "country_code": "RU", "city": "Москва", "provider": "Билайн"],
                       "ipv6_leak": false, "dns_protected": true, "checked_at": Date().timeIntervalSince1970])
        case "connect": vpn = "connected"; emitState(); return ok(NSNull())
        case "disconnect": vpn = "off"; emitState(); return ok(NSNull())
        case "select_server":
            server = args?["name"] as? String ?? server
            emitState()
            return ok(NSNull())
        case "set_routing":
            routing = args?["routing"] as? String ?? routing
            return ok(settings())
        default: return ok(NSNull())
        }
    }

    private func status() -> [String: Any] {
        var st: [String: Any] = ["vpn": vpn, "mode": "tun", "routing": routing, "server": server,
                                 "connection": ["id": "sub1", "name": "Моя подписка"]]
        if vpn != "off" { st["since"] = since }
        if vpn == "reconnecting" { st["attempt"] = [2, 3] }
        if vpn == "error" { st["error"] = "core.start_failed" }
        return st
    }

    private func settings() -> [String: Any] {
        let gb = 1_073_741_824.0
        return [
            "routing": routing, "blocked_preset": true, "active_connection": "sub1",
            "russia_direct": ["domains": true, "ips": true], "on_server_down": "reconnect", "auto_update": true,
            "restore_on_logon": true, "servers_by_country": false,
            "appearance": ["theme": ProcessInfo.processInfo.environment["KLICK_MOCK_THEME"] ?? "system", "base": "graphite", "accent": "#34c759", "glass": false],
            "lists": ["all_vpn": [
                ["target": ["kind": "service", "value": "sber"], "route": "direct", "enabled": true],
                ["target": ["kind": "domain", "value": "gosuslugi.ru"], "route": "direct", "enabled": true],
            ], "selected": [["target": ["kind": "service", "value": "youtube"], "route": "vpn", "enabled": true]]],
            "connections": [
                ["id": "sub1", "name": "Моя подписка", "kind": "subscription", "selected_server": server,
                 "info": ["upload": 2.2 * gb, "download": 33 * gb, "total": 100 * gb, "expire": Date().timeIntervalSince1970 + 28 * 86400]],
                ["id": "sub2", "name": "Рабочая", "kind": "subscription", "info": ["upload": 0, "download": 4 * gb, "total": 0]],
                ["id": "one", "name": "grpc", "kind": "single"],
            ],
            "kill_switch": ["enabled": true, "programs": [["folder": "/Applications/Telegram.app", "enabled": true]]],
        ]
    }

    private func servers() -> [[String: Any]] {
        [
            ["name": "Германия · Frankfurt", "kind": "vless", "delay": 39, "selected": server == "Германия · Frankfurt"],
            ["name": "Финляндия · Helsinki", "kind": "hysteria2", "delay": 48, "selected": server == "Финляндия · Helsinki"],
            ["name": "Нидерланды · Amsterdam", "kind": "vless", "delay": 61, "selected": server == "Нидерланды · Amsterdam"],
            ["name": "США · New York", "kind": "trojan", "delay": 142, "selected": server == "США · New York"],
            ["name": "Германия · Berlin", "kind": "hysteria2", "selected": false],
        ]
    }

    private func emitState() {
        emit(["t": "event", "ev": "state", "state": status()])
    }

    private func emit(_ obj: [String: Any]) {
        if let d = try? JSONSerialization.data(withJSONObject: obj) { onEvent?(d) }
    }

    private func ok(_ value: Any) -> ServiceLink.Reply {
        (true, (try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed)) ?? Data("null".utf8))
    }
}
