import Foundation
import NetworkExtension

/// VPN без приложения — для Пункта управления, виджета и Live Activity. Туннель запускает сама iOS по профилю,
/// который приложение уже сохранило; конфиг ядра лежит в общей папке, поэтому расширение VPN поднимается
/// и без движка. Приложение, когда его откроют, увидит включённый туннель и подхватит его (Host.swift).
enum VPNSwitch {
    enum Failure: LocalizedError {
        case noProfile

        var errorDescription: String? { "Сначала подключитесь один раз из kl!ck" }
    }

    static func manager() async -> NETunnelProviderManager? {
        try? await NETunnelProviderManager.loadAllFromPreferences().first
    }

    /// Включён ли VPN (или включается).
    static func isOn() async -> Bool {
        guard let m = await manager() else { return false }
        return [.connected, .connecting, .reasserting].contains(m.connection.status)
    }

    static func set(_ on: Bool) async throws {
        guard let m = await manager() else { throw Failure.noProfile }
        if on {
            if !m.isEnabled {
                m.isEnabled = true
                try await m.saveToPreferences()
                try await m.loadFromPreferences()
            }
            try m.connection.startVPNTunnel()
        } else {
            // Как в приложении: иначе «Восстанавливать подключение» сразу поднимет туннель снова.
            if m.isOnDemandEnabled {
                m.isOnDemandEnabled = false
                try await m.saveToPreferences()
            }
            if let core = AppGroup.core {
                try? Data("user".utf8).write(to: core.appendingPathComponent("stop-reason"))
            }
            m.connection.stopVPNTunnel()
        }
    }
}

/// Что показывает виджет: состояние, подключение и сервер. Пишет приложение (WidgetBridge), читает виджет.
struct WidgetState: Codable, Equatable {
    var vpn: String
    var name: String?
    var server: String?
    var since: Double?

    private static var file: URL? { AppGroup.directory?.appendingPathComponent("widget-state.json") }

    static func load() -> WidgetState? {
        guard let file, let data = try? Data(contentsOf: file) else { return nil }
        return try? JSONDecoder().decode(WidgetState.self, from: data)
    }

    func save() {
        guard let file = Self.file, let data = try? JSONEncoder().encode(self) else { return }
        try? data.write(to: file, options: .atomic)
    }
}
