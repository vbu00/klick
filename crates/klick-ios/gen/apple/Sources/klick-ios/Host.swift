import Foundation
import NetworkExtension
import UIKit
import WebKit

// Функции C для движка kl!ck (Rust, crates/klick-ios/src/host.rs). Движок зовёт их со своих фоновых
// потоков и ждёт ответа; NetworkExtension отвечает через главный поток — поэтому ждать здесь можно,
// а звать эти функции с главного потока нельзя. Строки отдаются через strdup, освобождает Rust.

@_cdecl("klick_group_dir")
func klickGroupDir() -> UnsafeMutablePointer<CChar>? {
    AppGroup.directory.flatMap { strdup($0.path) }
}

@_cdecl("klick_device_id")
func klickDeviceId() -> UnsafeMutablePointer<CChar>? {
    onMain { UIDevice.current.identifierForVendor?.uuidString }.flatMap { strdup($0) }
}

@_cdecl("klick_clipboard")
func klickClipboard() -> UnsafeMutablePointer<CChar>? {
    onMain { UIPasteboard.general.string }.flatMap { strdup($0) }
}

/// Окно на весь экран: WKWebView по размеру окна и без автоматических отступов прокрутки. Вырез, строку
/// состояния и полоску «домой» обходит сама страница (`env(safe-area-inset-*)`), иначе внизу оставалась
/// полоса. Можно звать с любого потока: работа — на главном.
@_cdecl("klick_full_screen")
func klickFullScreen() {
    DispatchQueue.main.async {
        for case let scene as UIWindowScene in UIApplication.shared.connectedScenes {
            for window in scene.windows {
                guard let view = findWebView(in: window), let parent = view.superview else { continue }
                let before = view.frame
                view.scrollView.contentInsetAdjustmentBehavior = .never
                view.frame = parent.convert(window.bounds, from: window)
                // Под страницей WebKit красит цветом окна (тёмным) — пусть красит фоном самой страницы,
                // светлым или тёмным по теме.
                view.underPageBackgroundColor = nil
                NSLog("kl!ck: окно %@, WebView %@ → %@", NSCoder.string(for: window.bounds), NSCoder.string(for: before), NSCoder.string(for: view.frame))
            }
        }
    }
}

private func findWebView(in view: UIView) -> WKWebView? {
    if let web = view as? WKWebView { return web }
    for sub in view.subviews {
        if let web = findWebView(in: sub) { return web }
    }
    return nil
}

@_cdecl("klick_tunnel_start")
func klickTunnelStart() -> UnsafeMutablePointer<CChar>? {
    Tunnel.shared.start().flatMap { strdup($0) }
}

@_cdecl("klick_tunnel_reload")
func klickTunnelReload() -> UnsafeMutablePointer<CChar>? {
    Tunnel.shared.reload().flatMap { strdup($0) }
}

@_cdecl("klick_tunnel_stop")
func klickTunnelStop() {
    Tunnel.shared.stop()
}

@_cdecl("klick_tunnel_connected")
func klickTunnelConnected() -> Bool {
    Tunnel.shared.load()?.connection.status == .connected
}

private func onMain<T>(_ body: () -> T) -> T {
    Thread.isMainThread ? body() : DispatchQueue.main.sync(execute: body)
}

/// Профиль VPN kl!ck в Настройках iOS и его туннель — расширение KlickTunnel.
final class Tunnel {
    static let shared = Tunnel()

    private let lock = NSLock()
    private var manager: NETunnelProviderManager?
    private var observer: NSObjectProtocol?
    private var lastStatus: NEVPNStatus = .invalid

    /// Профиль kl!ck, если он уже есть в Настройках. Первый раз читается из настроек системы.
    func load() -> NETunnelProviderManager? {
        lock.lock()
        defer { lock.unlock() }
        if manager == nil {
            let done = DispatchSemaphore(value: 0)
            NETunnelProviderManager.loadAllFromPreferences { managers, _ in
                self.manager = managers?.first
                done.signal()
            }
            _ = done.wait(timeout: .now() + 10)
            if let manager { observe(manager) }
        }
        return manager
    }

    /// Поднять туннель и дождаться подключения. `nil` — подключён, иначе — почему нет.
    func start() -> String? {
        guard let provider = Self.extensionBundleId else { return "в приложении нет расширения VPN (KlickTunnel.appex)" }
        guard let core = AppGroup.core else {
            return "подпись без общей папки App Group: расширение VPN не увидит конфиг. Переподпишите kl!ck с App Groups (docs/ios-install.md)"
        }
        let manager = load() ?? NETunnelProviderManager()
        let proto = (manager.protocolConfiguration as? NETunnelProviderProtocol) ?? NETunnelProviderProtocol()
        proto.providerBundleIdentifier = provider
        proto.serverAddress = "kl!ck"
        proto.disconnectOnSleep = false
        manager.protocolConfiguration = proto
        manager.localizedDescription = "kl!ck"
        manager.isEnabled = true
        // «Восстанавливать подключение»: iOS сама поднимает туннель после перезагрузки и сбоя расширения.
        manager.onDemandRules = [NEOnDemandRuleConnect()]
        manager.isOnDemandEnabled = Self.preference("restore_on_logon")
        // Первый раз iOS спрашивает разрешение добавить профиль VPN — ждём, пока человек ответит.
        if let error = wait(seconds: 120, { manager.saveToPreferences(completionHandler: $0) }) {
            return "профиль VPN не сохранён: \(error.localizedDescription)"
        }
        // Только что сохранённый профиль нужно перечитать, иначе первый запуск туннеля не проходит.
        _ = wait(seconds: 10, { manager.loadFromPreferences(completionHandler: $0) })
        lock.lock()
        self.manager = manager
        observe(manager)
        lock.unlock()
        try? FileManager.default.removeItem(at: core.appendingPathComponent("last-error"))
        do {
            try manager.connection.startVPNTunnel()
        } catch {
            return "туннель не запустился: \(error.localizedDescription)"
        }
        let started = Date()
        var moving = false
        while Date().timeIntervalSince(started) < 30 {
            switch manager.connection.status {
            case .connected:
                return nil
            case .connecting, .reasserting:
                moving = true
            case .disconnected, .invalid:
                if moving || Date().timeIntervalSince(started) > 5 {
                    return Self.lastError(core) ?? "туннель не подключился"
                }
            default:
                break
            }
            Thread.sleep(forTimeInterval: 0.1)
        }
        return "туннель не подключился за 30 с"
    }

    /// Расширение перечитывает конфиг целиком — вместе со входом управления ядра.
    func reload() -> String? {
        guard let session = load()?.connection as? NETunnelProviderSession, session.status == .connected else { return "туннель не подключён" }
        var reply: String? = "расширение VPN не ответило"
        let done = DispatchSemaphore(value: 0)
        do {
            try session.sendProviderMessage(Data("reload".utf8)) { data in
                let text = data.map { String(decoding: $0, as: UTF8.self) } ?? ""
                reply = text.isEmpty ? nil : text
                done.signal()
            }
        } catch {
            return error.localizedDescription
        }
        _ = done.wait(timeout: .now() + 20)
        return reply
    }

    func stop() {
        guard let manager = load() else { return }
        // Иначе iOS сразу поднимет туннель снова.
        if manager.isOnDemandEnabled {
            manager.isOnDemandEnabled = false
            _ = wait(seconds: 10, { manager.saveToPreferences(completionHandler: $0) })
        }
        manager.connection.stopVPNTunnel()
        let started = Date()
        while manager.connection.status != .disconnected, manager.connection.status != .invalid, Date().timeIntervalSince(started) < 5 {
            Thread.sleep(forTimeInterval: 0.1)
        }
    }

    /// Состояние туннеля — движку: подключился (в том числе из Настроек) или остановился.
    private func observe(_ manager: NETunnelProviderManager) {
        guard observer == nil else { return }
        lastStatus = manager.connection.status
        observer = NotificationCenter.default.addObserver(forName: .NEVPNStatusDidChange, object: nil, queue: .main) { [weak self] _ in
            guard let self, let status = self.manager?.connection.status else { return }
            let was = self.lastStatus
            self.lastStatus = status
            if status == .connected, was != .connected {
                klick_tunnel_changed(true, false)
            } else if status == .disconnected || status == .invalid, was == .connected || was == .disconnecting || was == .reasserting {
                let reason = AppGroup.core.map { $0.appendingPathComponent("stop-reason") }
                let byUser = reason.flatMap { try? String(contentsOf: $0, encoding: .utf8) } == "user"
                if let reason { try? FileManager.default.removeItem(at: reason) }
                klick_tunnel_changed(false, byUser)
            }
        }
    }

    /// Идентификатор расширения в этой подписи: подписчики меняют его под свой аккаунт.
    private static var extensionBundleId: String? {
        guard let plugins = Bundle.main.builtInPlugInsURL,
              let appex = (try? FileManager.default.contentsOfDirectory(at: plugins, includingPropertiesForKeys: nil))?.first(where: { $0.pathExtension == "appex" })
        else { return nil }
        return Bundle(url: appex)?.bundleIdentifier
    }

    /// Почему расширение не подняло туннель: оно пишет это в общую папку.
    private static func lastError(_ core: URL) -> String? {
        (try? String(contentsOf: core.appendingPathComponent("last-error"), encoding: .utf8)).flatMap { $0.isEmpty ? nil : $0 }
    }

    /// Настройка kl!ck из settings.json движка.
    private static func preference(_ key: String) -> Bool {
        let file = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0].appendingPathComponent("klick/settings.json")
        guard let data = try? Data(contentsOf: file), let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return false }
        return json[key] as? Bool ?? false
    }

    private func wait(seconds: Double, _ call: (@escaping (Error?) -> Void) -> Void) -> Error? {
        var result: Error?
        let done = DispatchSemaphore(value: 0)
        call { error in
            result = error
            done.signal()
        }
        if done.wait(timeout: .now() + seconds) == .timedOut {
            return NSError(domain: "klick", code: 1, userInfo: [NSLocalizedDescriptionKey: "система не ответила за \(Int(seconds)) с"])
        }
        return result
    }
}
