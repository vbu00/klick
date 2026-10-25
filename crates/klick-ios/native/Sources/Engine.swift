import Foundation
import SwiftUI

/// Ошибка движка: код (`conn.exists`, `sub.refused`…) и подробности.
struct EngineError: Error {
    let code: String
    let params: [String: Any]
}

private final class Pending {
    let continuation: CheckedContinuation<(Data, Bool), Never>
    init(_ continuation: CheckedContinuation<(Data, Bool), Never>) { self.continuation = continuation }
}

/// Ответ Rust на команду — с его потока; строка живёт только до возврата, поэтому копируется сразу.
private let onReply: klick_callback = { ctx, json, ok in
    guard let ctx else { return }
    let pending = Unmanaged<Pending>.fromOpaque(ctx).takeRetainedValue()
    let data = json.map { Data(String(cString: $0).utf8) } ?? Data("null".utf8)
    pending.continuation.resume(returning: (data, ok))
}

/// Событие движка (состояние, скорость, уведомление…) — на главный поток.
private let onEvent: klick_callback = { _, json, _ in
    guard let json else { return }
    let data = Data(String(cString: json).utf8)
    Task { @MainActor in Engine.shared.handle(event: data) }
}

/// Движок kl!ck (Rust, crates/klick-native) для интерфейса: те же команды, что у веб-окна, и события.
@MainActor
final class Engine: ObservableObject {
    static let shared = Engine()

    @Published private(set) var up = false
    @Published private(set) var startError: String?
    @Published private(set) var state: StateView?
    @Published private(set) var settings: Settings?
    @Published private(set) var servers: [String: [Server]] = [:]
    @Published private(set) var traffic = Traffic()
    @Published private(set) var pinging = false
    @Published private(set) var speed: SpeedView?
    @Published private(set) var speedTesting = false
    @Published private(set) var powerPending = false
    @Published var toast: Toast?

    private var started = false
    private let decoder: JSONDecoder = {
        let d = JSONDecoder()
        d.keyDecodingStrategy = .convertFromSnakeCase
        return d
    }()

    var isRunning: Bool {
        ["connecting", "connected", "reconnecting", "server_down"].contains(state?.vpn ?? "off")
    }

    var activeId: String? { settings?.activeConnection }

    // MARK: Запуск и команды

    /// Движок поднимается в фоне: Swift ждёт ответов NetworkExtension через главный поток.
    func start() {
        guard !started else { return }
        started = true
        Task.detached(priority: .userInitiated) {
            let raw = klick_native_start()
            let error = raw.map { String(cString: $0) }
            if let raw { klick_native_free(raw) }
            await Engine.shared.didStart(error: error)
        }
    }

    private func didStart(error: String?) async {
        if let error {
            startError = error
            return
        }
        up = true
        _ = klick_native_subscribe(nil, onEvent)
        await reload()
        await selfTest()
    }

    @discardableResult
    func call(_ cmd: String, _ args: [String: Any]? = nil) async throws -> Data {
        var request: [String: Any] = ["cmd": cmd]
        if let args { request["args"] = args }
        let body = try JSONSerialization.data(withJSONObject: request)
        let text = String(decoding: body, as: UTF8.self)
        let (data, ok) = await withCheckedContinuation { (continuation: CheckedContinuation<(Data, Bool), Never>) in
            let ctx = Unmanaged.passRetained(Pending(continuation)).toOpaque()
            text.withCString { klick_native_call($0, ctx, onReply) }
        }
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
        if let s = try? await get(Settings.self, "settings") { settings = s }
        await loadServers()
    }

    func loadServers() async {
        guard let id = activeId else { return }
        if let list = try? await get([Server].self, "servers") { servers[id] = list }
    }

    /// Серверы любой подписки — не переключаясь на неё.
    func viewServers(_ id: String) async {
        if id == activeId {
            await loadServers()
        } else if let list = try? await get([Server].self, "connection_servers", ["id": id]) {
            servers[id] = list
        }
    }

    func handle(event data: Data) {
        guard let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any], let ev = obj["ev"] as? String else { return }
        switch ev {
        case "state":
            guard let raw = obj["state"], let d = try? JSONSerialization.data(withJSONObject: raw), let st = try? decoder.decode(StateView.self, from: d) else { return }
            let switched = st.connection?.id != state?.connection?.id
            state = st
            if switched { Task { await reload() } }
        case "traffic":
            traffic = Traffic(up: (obj["up"] as? NSNumber)?.doubleValue ?? 0, down: (obj["down"] as? NSNumber)?.doubleValue ?? 0)
        case "settings":
            Task { if let s = try? await get(Settings.self, "settings") { settings = s } }
        case "servers":
            if let id = obj["connection"] as? String { Task { await viewServers(id) } }
        case "notice":
            if let code = obj["code"] as? String, let text = Texts.notice(code, obj["params"] as? [String: Any] ?? [:]) {
                toast = Toast(text: text, bad: Texts.noticeIsBad(code))
            }
        default:
            break
        }
    }

    // MARK: Действия

    /// Кнопка питания. `viewing` — подписка на экране: без VPN она становится активной и подключается.
    func power(viewing: String?) async {
        guard !powerPending else { return }
        powerPending = true
        defer { powerPending = false }
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
        do { servers[id] = try await get([Server].self, "test_latency") } catch { show(error) }
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
            toast = Toast(text: "Подключение добавлено")
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
            toast = Toast(text: "Подписка обновлена")
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
        if let s = try? await get(Settings.self, "settings") { settings = s }
    }

    func setPref(_ key: String, _ value: Any) async {
        do { settings = try await get(Settings.self, "set_preferences", ["prefs": [key: value]]) } catch { show(error) }
    }

    /// Симулятор в CI (scripts/ios/native-sim-test.sh): подписка из окружения — серверы и задержка видны на снимке.
    private func selfTest() async {
        guard let sub = ProcessInfo.processInfo.environment["KLICK_SELFTEST_SUB"], !sub.isEmpty else { return }
        var id = settings?.connections.first?.id
        if id == nil {
            do {
                let data = try await call("add_connection", ["source": sub, "name": "Проверка"])
                id = ((try? JSONSerialization.jsonObject(with: data)) as? [String: Any])?["id"] as? String
            } catch let error as EngineError where error.code == "conn.exists" {
                id = error.params["id"] as? String
            } catch {
                return
            }
        }
        guard let id else { return }
        await selectConnection(id)
        await testLatency()
    }
}
