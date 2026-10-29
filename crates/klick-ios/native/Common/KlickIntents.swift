import AppIntents
import Foundation
import WidgetKit
#if canImport(ActivityKit)
import ActivityKit
#endif

/// «Включить или выключить kl!ck»: кнопка в Пункте управления, на виджете и в Live Activity.
@available(iOS 17.0, *)
struct ToggleVPNIntent: SetValueIntent {
    static let title: LocalizedStringResource = "Включить или выключить kl!ck"
    static let description = IntentDescription("Включает и выключает VPN kl!ck, не открывая приложение.")

    @Parameter(title: "VPN")
    var value: Bool

    init() {}

    init(value: Bool) {
        self.value = value
    }

    func perform() async throws -> some IntentResult {
        try await VPNSwitch.set(value)
        WidgetCenter.shared.reloadAllTimelines()
        if #available(iOS 18.0, *) { ControlCenter.shared.reloadAllControls() }
        return .result()
    }
}

/// Live Activity и Dynamic Island: при беде со связью или всё время с VPN — как выбрано в настройках.
@available(iOS 16.1, *)
struct KlickActivityAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        /// `connected`, `reconnecting`, `server_down`.
        var vpn: String
        var server: String?
        var since: Date
        /// Что в свёрнутом острове, пока всё хорошо (Prefs.islandLeft / islandRight).
        var left: String
        var right: String
        var ping: Int?
        var flag: String?
    }

    /// Подключение (подписка).
    var name: String
}
