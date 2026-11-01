import AppKit
import Foundation
import SwiftUI

/// Ошибка службы: код (`conn.exists`, `sub.refused`…) и подробности.
struct EngineError: Error {
    let code: String
    let params: [String: Any]
}

/// Откуда ответы: служба kl!ck по сокету или тестовые данные (`KLICK_MOCK=1` — снимки в CI).
protocol Backend: AnyObject {
    var onEvent: ((Data) -> Void)? { get set }
    var onLink: ((Bool) -> Void)? { get set }
    func start()
    func call(_ cmd: String, _ args: [String: Any]?) async -> ServiceLink.Reply
}

extension ServiceLink: Backend {}

/// Что есть только у службы на Mac: способ подключения и Kill Switch.
struct MacExtras: Decodable, Equatable {
    struct KillSwitch: Decodable, Equatable {
        struct Program: Decodable, Equatable {
            var folder: String
            var enabled: Bool
        }
        var enabled: Bool
        var programs: [Program]
    }
    var killSwitch: KillSwitch?
    var onExit: String?
}

/// Состояние окна: то же, что у iOS β (crates/klick-ios/native/Sources/Engine.swift), но движок — служба.
@MainActor
final class Engine: ObservableObject {
    static let shared = Engine()

    @Published private(set) var up = false
    @Published private(set) var state: StateView?
    @Published var settings: Settings?
    @Published private(set) var extras: MacExtras?
    @Published private(set) var servers: [String: [Server]] = [:]
    @Published private(set) var traffic = Traffic()
    @Published private(set) var pinging = false
    @Published private(set) var tested: Set<String> = []
    @Published private(set) var speed: SpeedView?
    @Published private(set) var speedTesting = false
    @Published private(set) var pending: String?
    @Published var toast: Toast?
    @Published var focusConnection: String?
    @Published private(set) var catalog: [Service] = []

    let mock = ProcessInfo.processInfo.environment["KLICK_MOCK"] == "1"
    private lazy var backend: Backend = mock ? MockBackend() : ServiceLink()
    private var started = false
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    var isRunning: Bool { ["connecting", "connected", "reconnecting", "server_down"].contains(state?.vpn ?? "off") }
    var activeId: String? { settings?.activeConnection }
    var mode: String { state?.mode ?? "tun" }

    // MARK: Канал и команды

    func start() {
        guard !started else { return }
        started = true
        backend.onEvent = { data in Task { @MainActor in Engine.shared.handle(event: data) } }
        backend.onLink = { up in Task { @MainActor in await Engine.shared.link(up) } }
        backend.start()
    }

    private func link(_ isUp: Bool) async {
        up = isUp
        guard isUp else { return }
        _ = await backend.call("subscribe", nil)
        await reload()
        await loadCatalog()
    }

    @discardableResult
    func call(_ cmd: String, _ args: [String: Any]? = nil) async throws -> Data {
        let (ok, data) = await backend.call(cmd, args)
        if ok { return data }
        let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        throw EngineError(code: obj["code"] as? String ?? "unknown", params: obj["params"] as? [String: Any] ?? [:])
    }

    func get<T: Decodable>(_ type: T.Type, _ cmd: String, _ args: [String: Any]? = nil) async throws -> T {
        try decoder.decode(T.self, from: try await call(cmd, args))
    }

    func show(_ error: Error) {
        let code = (error as? EngineError)?.code ?? "unknown"
        toast = Toast(text: Texts.error(code), bad: true)
    }

    // MARK: Состояние

    func reload() async {
        if let st = try? await get(StateView.self, "status") { state = st }
        await reloadSettings()
        await loadServers()
    }

    private func reloadSettings() async {
        guard let data = try? await call("settings") else { return }
        if let s = try? decoder.decode(Settings.self, from: data) { settings = s }
        if let x = try? decoder.decode(MacExtras.self, from: data) { extras = x }
    }

    func loadServers() async {
        guard let id = activeId else { return }
        if let list = try? await get([Server].self, "servers") { servers[id] = list }
    }

    func viewServers(_ id: String) async {
        if id == activeId {
            await loadServers()
        } else if let list = try? await get([Server].self, "connection_servers", ["id": id]) {
            servers[id] = list
        }
    }

    func loadCatalog() async {
        if catalog.isEmpty, let c = try? await get(Catalog.self, "catalog") { catalog = c.services }
    }

    func handle(event data: Data) {
        guard let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], let ev = obj["ev"] as? String else { return }
        switch ev {
        case "state":
            guard let raw = obj["state"], let d = try? JSONSerialization.data(withJSONObject: raw), let st = try? decoder.decode(StateView.self, from: d) else { return }
            let switched = st.connection?.id != state?.connection?.id
            state = st
            if st.vpn != "connected" && traffic != Traffic() { traffic = Traffic() }
            if switched { Task { await reload() } }
        case "traffic":
            traffic.push(up: (obj["up"] as? NSNumber)?.doubleValue ?? 0, down: (obj["down"] as? NSNumber)?.doubleValue ?? 0)
        case "settings":
            Task { await reloadSettings() }
        case "servers":
            if let id = obj["connection"] as? String {
                if id == activeId { tested.insert(id) }
                Task { await viewServers(id) }
            }
        case "notice":
            if let code = obj["code"] as? String, let text = Texts.notice(code, obj["params"] as? [String: Any] ?? [:]) {
                toast = Toast(text: text, bad: Texts.noticeIsBad(code))
            }
        default:
            break
        }
    }

    // MARK: Действия

    func power(viewing: String?) async {
        guard pending == nil else { return }
        pending = isRunning ? "off" : "on"
        defer { pending = nil }
        do {
            if isRunning {
                try await call("disconnect")
            } else {
                if let id = viewing, id != activeId { try await call("select_connection", ["id": id]) }
                try await call("connect")
            }
        } catch {
            show(error)
        }
        await reload()
    }

    @discardableResult
    func selectConnection(_ id: String) async -> Bool {
        do {
            try await call("select_connection", ["id": id])
            await reload()
            return true
        } catch {
            show(error)
            return false
        }
    }

    func selectServer(_ name: String) async {
        if let id = activeId, var list = servers[id] {
            for i in list.indices { list[i].selected = list[i].name == name }
            servers[id] = list
        }
        do { try await call("select_server", ["name": name]) } catch { show(error) }
        await loadServers()
    }

    func testLatency() async {
        guard let id = activeId, !pinging else { return }
        pinging = true
        defer { pinging = false }
        do {
            servers[id] = try await get([Server].self, "test_latency")
            tested.insert(id)
        } catch {
            show(error)
        }
    }

    func speedTest() async {
        guard !speedTesting else { return }
        speedTesting = true
        defer { speedTesting = false }
        do {
            speed = try await get(SpeedView.self, "speed_test")
            await loadServers()
        } catch {
            show(error)
        }
    }

    func add(source: String, name: String) async -> Bool {
        do {
            try await call("add_connection", ["source": source, "name": name.isEmpty ? NSNull() : name])
            toast = Toast("Подключение добавлено", "Проверьте задержку серверов")
            await reload()
            return true
        } catch let error as EngineError where error.code == "conn.exists" {
            toast = Toast("Уже добавлено", error.params["name"] as? String, tone: "dim")
            focusConnection = error.params["id"] as? String
            return true
        } catch {
            show(error)
            return false
        }
    }

    func importFile(fileName: String, content: String) async -> Bool {
        do {
            try await call("import_file", ["file_name": fileName, "content": content])
            toast = Toast("Файл импортирован", fileName)
            await reload()
            return true
        } catch {
            show(error)
            return false
        }
    }

    func refresh(_ id: String) async {
        do {
            try await call("refresh_connection", ["id": id])
            toast = Toast("Подписка обновлена")
        } catch {
            show(error)
        }
        await reload()
    }

    func remove(_ id: String) async {
        do { try await call("remove_connection", ["id": id]) } catch { show(error) }
        await reload()
    }

    func setRouting(_ routing: String) async {
        do { try await call("set_routing", ["routing": routing]) } catch { show(error) }
        await reloadSettings()
    }

    func setMode(_ mode: String) async {
        do { try await call("set_mode", ["mode": mode]) } catch { show(error) }
        await reload()
    }

    /// «Российские сайты и сервисы» одним тумблером: и .ru/.рф, и российские адреса.
    func setRussia(_ on: Bool) async {
        await setPrefs(["ru_domains": on, "ru_ips": on])
    }

    func setPref(_ key: String, _ value: Any) async {
        await setPrefs([key: value])
    }

    func setPrefs(_ prefs: [String: Any]) async {
        do { try await call("set_preferences", ["prefs": prefs]) } catch { show(error) }
        await reloadSettings()
    }

    func setAppearance(_ a: Appearance) {
        settings?.appearance = a
        Task { await setPref("appearance", a.json) }
    }

    func setKillSwitch(_ on: Bool) async {
        do { try await call("kill_switch_set", ["enabled": on]) } catch { show(error) }
        await reloadSettings()
    }

    // MARK: Мой список

    func addRules(_ position: String, _ rules: [[String: Any]]) async -> Int {
        var added = 0
        var firstError: Error?
        for rule in rules {
            do {
                try await call("list_add", ["position": position, "rule": rule])
                added += 1
            } catch {
                if firstError == nil { firstError = error }
            }
        }
        if let firstError { show(firstError) }
        await reloadSettings()
        return added
    }

    func removeRule(_ position: String, _ index: Int) async {
        do {
            try await call("list_remove", ["position": position, "index": index])
            toast = Toast("Правило удалено", tone: "dim")
        } catch {
            show(error)
        }
        await reloadSettings()
    }

    func setRuleEnabled(_ position: String, _ index: Int, _ enabled: Bool) async {
        do { try await call("list_set_enabled", ["position": position, "index": index, "enabled": enabled]) } catch { show(error) }
        await reloadSettings()
    }

    /// Один тумблер «через VPN»: правило для цели в мой список текущего положения.
    func routeTarget(_ target: Rule.Target, vpn: Bool, label: String) async {
        guard let s = settings else { return }
        let position = s.routing
        let list = position == "all_vpn" ? s.lists?.allVpn ?? [] : s.lists?.selected ?? []
        let route = vpn ? "vpn" : "direct"
        do {
            if let index = list.firstIndex(where: { $0.target.kind == target.kind && $0.target.value.lowercased() == target.value.lowercased() }) {
                let r = list[index]
                if r.route == route && r.enabled != false { return }
                if r.route != route { try await call("list_set_route", ["position": position, "index": index, "route": route]) }
                if r.enabled == false { try await call("list_set_enabled", ["position": position, "index": index, "enabled": true]) }
            } else {
                try await call("list_add", ["position": position, "rule": Rule.json(target, route: route)])
            }
            toast = Toast("\(label) → \(vpn ? "через VPN" : "напрямую")")
        } catch {
            show(error)
        }
        await reloadSettings()
    }

    // MARK: Сейчас в сети и диагностика

    func connections() async -> [LiveConn]? { try? await get([LiveConn].self, "connections") }
    func failures() async -> [Failure]? { try? await get([Failure].self, "failures") }
    func log() async -> [LogLine] { (try? await get([LogLine].self, "log")) ?? [] }
    func about() async -> About? { try? await get(About.self, "about") }

    func ipCheck() async -> IpReport? {
        do { return try await get(IpReport.self, "ip_check") } catch {
            show(error)
            return nil
        }
    }

    func report() async -> String? {
        guard let data = try? await call("report") else { return nil }
        return (try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed)) as? String
    }
}
