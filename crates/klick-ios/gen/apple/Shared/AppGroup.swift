import Foundation

/// Общая папка приложения и расширения VPN (App Group): приложение кладёт туда конфиг ядра, серверы и
/// наборы, расширение их читает и отвечает тем же. Подписчики (AltStore, SideStore, Sideloadly, ESign…)
/// переписывают имя группы под свой аккаунт, поэтому оно берётся из подписи, а не из кода: права
/// в подписи, затем подсказка AltStore/SideStore в Info.plist, в конце — имя из сборки.
enum AppGroup {
    static let id: String? = fromEntitlements() ?? fromAltStore() ?? (Bundle.main.object(forInfoDictionaryKey: "KlickAppGroup") as? String)

    static var directory: URL? {
        guard let id else { return nil }
        return FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: id)
    }

    /// Домашняя папка ядра: конфиг, серверы, наборы, база стран.
    static var core: URL? {
        directory?.appendingPathComponent("core", isDirectory: true)
    }

    private static func fromAltStore() -> String? {
        (Bundle.main.object(forInfoDictionaryKey: "ALTAppGroups") as? [String])?.first
    }

    /// `SecTaskCopyValueForEntitlement` есть в Security.framework и на iOS, но не в его заголовках.
    private static func fromEntitlements() -> String? {
        typealias CreateFromSelf = @convention(c) (CFAllocator?) -> Unmanaged<CFTypeRef>?
        typealias CopyValue = @convention(c) (CFTypeRef, CFString, UnsafeMutablePointer<Unmanaged<CFError>?>?) -> Unmanaged<CFTypeRef>?
        guard let process = dlopen(nil, RTLD_NOW),
              let create = dlsym(process, "SecTaskCreateFromSelf"),
              let copy = dlsym(process, "SecTaskCopyValueForEntitlement"),
              let task = unsafeBitCast(create, to: CreateFromSelf.self)(nil)?.takeRetainedValue(),
              let value = unsafeBitCast(copy, to: CopyValue.self)(task, "com.apple.security.application-groups" as CFString, nil)?.takeRetainedValue()
        else { return nil }
        return (value as? [String])?.first
    }
}
