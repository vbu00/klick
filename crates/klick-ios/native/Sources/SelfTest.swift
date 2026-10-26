import Foundation

/// Полная самопроверка для CI (scripts/ios/native-sim-test.sh, `KLICK_SELFTEST_FULL=1`): каждая команда
/// движка, которой пользуются экраны, — через мост Swift → Rust; ответ должен разобраться моделями
/// интерфейса, а действие — поменять настройки и вернуться как было. Туннель VPN симулятор не поднимает,
/// поэтому команды «только под VPN» проверяются на понятный отказ. Итог — Library/Application Support/
/// klick/logs/selftest.log: строки `ok …` / `FAIL …` и в конце `selftest done`.
extension Engine {
    func fullSelfTest() async {
        var lines: [String] = []
        func check(_ name: String, _ ok: Bool, _ detail: String = "") {
            lines.append("\(ok ? "ok  " : "FAIL") \(name)\(detail.isEmpty ? "" : " — \(detail)")")
        }
        func attempt<T: Decodable>(_ name: String, _ type: T.Type, _ cmd: String, _ args: [String: Any]? = nil) async -> T? {
            do {
                return try await get(type, cmd, args)
            } catch {
                check(name, false, describe(error))
                return nil
            }
        }
        func describe(_ error: Error) -> String {
            if let e = error as? EngineError { return "код \(e.code)" }
            return "\(error)"
        }
        /// Отказ с этим кодом — правильный ответ (например, «включите VPN»).
        func expectRefusal(_ name: String, _ cmd: String, _ codes: [String]) async {
            do {
                try await call(cmd)
                check(name, true, "выполнено")
            } catch let e as EngineError where codes.contains(e.code) {
                check(name, true, "отказ \(e.code), как и должно быть без VPN")
            } catch {
                check(name, false, describe(error))
            }
        }

        // Чтение: всё, что показывают экраны.
        if let st = await attempt("status", StateView.self, "status") { check("status", true, "vpn \(st.vpn)") }
        guard let s0 = await attempt("settings", Settings.self, "settings") else { return finish(lines) }
        check("settings", true, "подключений \(s0.connections.count), тема \(s0.appearance?.theme ?? "нет")")
        if let list = await attempt("servers", [Server].self, "servers") { check("servers", !list.isEmpty, "\(list.count) шт.") }
        if let id = s0.activeConnection, let list = await attempt("connection_servers", [Server].self, "connection_servers", ["id": id]) {
            check("connection_servers", !list.isEmpty, "\(list.count) шт.")
        }
        let catalog = await attempt("catalog", Catalog.self, "catalog")?.services ?? []
        check("catalog", !catalog.isEmpty, "\(catalog.count) сервисов")
        if let a = await attempt("about", About.self, "about") { check("about", !a.version.isEmpty, "версия \(a.version)") }
        if let l = await attempt("log", [LogLine].self, "log") { check("log", true, "\(l.count) записей") }
        let report = await report()
        check("report", !(report ?? "").isEmpty, "\((report ?? "").count) символов")
        if let c = await attempt("connections", [LiveConn].self, "connections") { check("connections", true, "\(c.count) шт.") }
        if let f = await attempt("failures", [Failure].self, "failures") { check("failures", true, "\(f.count) шт.") }
        if let ip = await attempt("ip_check", IpReport.self, "ip_check") {
            check("ip_check", ip.direct.ipv4 != nil || ip.direct.ipv6 != nil || ip.direct.error != nil, "напрямую \(ip.direct.ipv4 ?? ip.direct.error ?? "—")")
        }
        await expectRefusal("speed_test", "speed_test", ["speed.vpn_off"])

        // Действия: поменять — проверить — вернуть.
        if let id = s0.activeConnection, s0.connections.first(where: { $0.id == id })?.kind == "subscription" {
            do {
                try await call("refresh_connection", ["id": id])
                check("refresh_connection", true)
            } catch {
                check("refresh_connection", false, describe(error))
            }
        }

        let other = s0.routing == "selected" ? "all_vpn" : "selected"
        await setRouting(other)
        check("set_routing", settings?.routing == other, "\(s0.routing) → \(settings?.routing ?? "?")")
        await setRouting(s0.routing)
        check("set_routing обратно", settings?.routing == s0.routing)

        let ru = s0.russiaDirect?.domains ?? false
        await setPref("ru_domains", !ru)
        check("ru_domains", settings?.russiaDirect?.domains == !ru)
        await setPref("ru_domains", ru)

        let down = s0.onServerDown ?? "reconnect"
        await setPrefs(["on_server_down": down == "fastest" ? "reconnect" : "fastest"])
        check("on_server_down", settings?.onServerDown != down, settings?.onServerDown ?? "?")
        await setPrefs(["on_server_down": down])

        let byCountry = s0.serversByCountry ?? false
        await setPrefs(["servers_by_country": !byCountry])
        check("servers_by_country", settings?.serversByCountry == !byCountry)
        await setPrefs(["servers_by_country": byCountry])

        let look = s0.appearance ?? .standard
        await setPrefs(["appearance": Appearance(theme: "custom", base: "midnight", accent: "#0a84ff", glass: !look.glass).json])
        let got = settings?.appearance
        check("appearance", got?.theme == "custom" && got?.base == "midnight" && got?.accent == "#0a84ff", "\(got?.theme ?? "?") \(got?.base ?? "?") \(got?.accent ?? "?")")
        await setPrefs(["appearance": look.json])
        check("appearance обратно", settings?.appearance == look)

        // Мой список: сервис из каталога — выключить — удалить; сайт тумблером «через VPN».
        let position = s0.routing
        func list() -> [Rule] { position == "all_vpn" ? settings?.lists?.allVpn ?? [] : settings?.lists?.selected ?? [] }
        let before = list().count
        if let svc = catalog.first(where: { s in !list().contains { $0.target.value == s.id } }) {
            let added = await addRules(position, [Rule.json(Rule.Target(kind: "service", value: svc.id), route: position == "selected" ? "vpn" : "direct")])
            check("list_add", added == 1 && list().count == before + 1, svc.name)
            if let i = list().firstIndex(where: { $0.target.value == svc.id }) {
                await setRuleEnabled(position, i, false)
                check("list_set_enabled", list()[i].enabled == false)
                await removeRule(position, i)
                check("list_remove", list().count == before)
            }
        }
        await routeTarget(Rule.Target(kind: "domain", value: "klick-selftest.example"), vpn: true, label: "проверка")
        if let i = list().firstIndex(where: { $0.target.value == "klick-selftest.example" }) {
            check("тумблер «через VPN»", list()[i].route == "vpn")
            await removeRule(position, i)
        } else {
            check("тумблер «через VPN»", false, "правило не появилось")
        }
        check("мой список как был", list().count == before)

        // Список ссылок текстом — как файл (import_file), потом удалить.
        let text = "vless://00000000-0000-0000-0000-000000000000@127.0.0.9:443?security=none#Один\nvless://00000000-0000-0000-0000-000000000001@127.0.0.9:444?security=none#Два"
        let ids = Set(s0.connections.map(\.id))
        if await importText(name: "Самопроверка", content: text), let new = settings?.connections.first(where: { !ids.contains($0.id) }) {
            check("import_file", true, new.name)
            await remove(new.id)
            check("remove_connection", settings?.connections.contains { $0.id == new.id } == false)
        } else {
            check("import_file", false, "подключение не появилось")
        }
        if let active = s0.activeConnection { await selectConnection(active) }

        // Разбор на стороне Swift: страны и поле «Добавить».
        let names: [(String, String, String?, String)] = [
            ("🇳🇱 NL · VLESS · XHTTP", "vless", "NL", "VLESS · XHTTP"),
            ("Германия · Frankfurt", "vless", "DE", "Frankfurt"),
            ("[US] New York", "trojan", "US", "New York"),
            ("🇪🇪 EE · AWG3.1", "wireguard", "EE", "AmneziaWG 3.1"),
            ("Amsterdam", "hysteria2", "NL", "Amsterdam"),
        ]
        for (name, kind, code, title) in names {
            let p = Countries.parse(name, kind)
            check("страна «\(name)»", p.code == code && p.title == title, "\(p.code ?? "—") · \(p.title)")
        }
        let detect: [(String, String)] = [
            ("https://panel.example/sub/abc", "sub"), ("vless://x@h:443#a", "link"), ("ssconf://x", "sub"),
            ("vless://a@b:1#x\nvless://c@d:2#y", "text"), ("proxies:\n  - name: x", "text"), ("привет", "bad"), ("", "empty"),
        ]
        for (input, kind) in detect {
            check("поле «Добавить»: \(kind)", Texts.detect(input).kind == kind, Texts.detect(input).kind)
        }
        finish(lines)
    }

    private func finish(_ lines: [String]) {
        let home = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        let file = home.appendingPathComponent("klick/logs/selftest.log")
        try? FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try? (lines + ["selftest done"]).joined(separator: "\n").appending("\n").write(to: file, atomically: true, encoding: .utf8)
    }
}
