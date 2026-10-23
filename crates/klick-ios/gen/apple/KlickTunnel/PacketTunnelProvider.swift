import NetworkExtension

/// Туннель kl!ck. Система отдаёт сюда пакеты устройства, ядро mihomo (библиотека Go,
/// crates/klick-ios/core) ведёт их по правилам kl!ck. Конфиг готовит движок в приложении и кладёт в общую
/// папку App Group; расширение работает и без приложения — когда VPN включают в Настройках или iOS
/// поднимает его сама («Восстанавливать подключение»).
final class PacketTunnelProvider: NEPacketTunnelProvider {
    private var pathObserver: NSKeyValueObservation?
    private var lastPath: NWPath?

    override func startTunnel(options: [String: NSObject]?, completionHandler: @escaping (Error?) -> Void) {
        guard let core = AppGroup.core else {
            return fail("подпись без общей папки App Group: расширение не видит конфиг", nil, completionHandler)
        }
        try? FileManager.default.removeItem(at: core.appendingPathComponent("stop-reason"))
        let config = core.appendingPathComponent("config.yaml")
        guard FileManager.default.fileExists(atPath: config.path) else {
            return fail("конфига ещё нет: откройте kl!ck и подключитесь из приложения", core, completionHandler)
        }
        setTunnelNetworkSettings(Self.networkSettings()) { error in
            if let error {
                return self.fail("адаптер не настроился: \(error.localizedDescription)", core, completionHandler)
            }
            // 0 — ядро само найдёт дескриптор utun этого туннеля.
            if let message = Self.text(klickcore_start(core.path, config.path, 0)) {
                return self.fail(message, core, completionHandler)
            }
            try? FileManager.default.removeItem(at: core.appendingPathComponent("last-error"))
            self.lastPath = self.defaultPath
            self.pathObserver = self.observe(\.defaultPath) { provider, _ in provider.pathChanged() }
            completionHandler(nil)
        }
    }

    override func stopTunnel(with reason: NEProviderStopReason, completionHandler: @escaping () -> Void) {
        // Выключили в Настройках, другой VPN занял место, профиль удалён — приложение не включает туннель снова.
        let byUser: [NEProviderStopReason] = [.userInitiated, .configurationDisabled, .configurationRemoved, .superceded, .userLogout, .userSwitch]
        if let core = AppGroup.core {
            try? Data((byUser.contains(reason) ? "user" : "\(reason.rawValue)").utf8).write(to: core.appendingPathComponent("stop-reason"))
        }
        pathObserver = nil
        klickcore_stop()
        completionHandler()
    }

    /// Приложение просит перечитать конфиг. Ответ — пусто или текст ошибки.
    override func handleAppMessage(_ messageData: Data, completionHandler: ((Data?) -> Void)?) {
        guard String(decoding: messageData, as: UTF8.self) == "reload" else {
            completionHandler?(nil)
            return
        }
        completionHandler?(Data((Self.text(klickcore_reload()) ?? "").utf8))
    }

    /// Wi-Fi ↔ сотовая: соединения через прежнюю сеть мертвы, ядро закрывает их сразу, а не по тайм-ауту.
    private func pathChanged() {
        guard let path = defaultPath, path.status == .satisfied else { return }
        if let last = lastPath, path.isEqual(to: last) { return }
        let moved = lastPath != nil
        lastPath = path
        if moved {
            klickcore_network_changed()
        }
    }

    private func fail(_ message: String, _ core: URL?, _ completionHandler: (Error?) -> Void) {
        if let core {
            try? Data(message.utf8).write(to: core.appendingPathComponent("last-error"))
        }
        completionHandler(NSError(domain: "app.klick.tunnel", code: 1, userInfo: [NSLocalizedDescriptionKey: message]))
    }

    /// Строка от ядра (ошибка) — в Swift; память освобождает ядро.
    private static func text(_ p: UnsafeMutablePointer<CChar>?) -> String? {
        guard let p else { return nil }
        defer { klickcore_free(p) }
        return String(cString: p)
    }

    /// Адаптер: весь интернет — в туннель, локальная сеть и широковещание — мимо (AirDrop, AirPlay,
    /// принтеры). Адреса — как у mihomo по умолчанию: DNS-запросы ядро ловит на любом адресе, а
    /// подменные адреса сайтов (fake-ip, 198.18.0.0/16) попадают в туннель.
    private static func networkSettings() -> NEPacketTunnelNetworkSettings {
        let settings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")
        settings.mtu = 9000
        let v4 = NEIPv4Settings(addresses: ["198.18.0.1"], subnetMasks: ["255.255.255.252"])
        v4.includedRoutes = [NEIPv4Route.default()]
        v4.excludedRoutes = [
            ("10.0.0.0", "255.0.0.0"), ("172.16.0.0", "255.240.0.0"), ("192.168.0.0", "255.255.0.0"),
            ("169.254.0.0", "255.255.0.0"), ("224.0.0.0", "240.0.0.0"),
        ].map { NEIPv4Route(destinationAddress: $0.0, subnetMask: $0.1) }
        settings.ipv4Settings = v4
        let v6 = NEIPv6Settings(addresses: ["fdfe:dcba:9876::1"], networkPrefixLengths: [126])
        v6.includedRoutes = [NEIPv6Route.default()]
        v6.excludedRoutes = [("fe80::", 10), ("fc00::", 7), ("ff00::", 8)].map {
            NEIPv6Route(destinationAddress: $0.0, networkPrefixLength: NSNumber(value: $0.1))
        }
        settings.ipv6Settings = v6
        let dns = NEDNSSettings(servers: ["198.18.0.2"])
        dns.matchDomains = [""]
        settings.dnsSettings = dns
        return settings
    }
}
