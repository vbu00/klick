import ActivityKit
import Foundation
import WidgetKit

/// Приложение → виджеты: состояние для виджета и Пункта управления, Live Activity — как выбрано в настройках.
/// Live Activity iOS разрешает начать только из открытого приложения, а в фоне kl!ck не работает:
/// беду, случившуюся при свёрнутом приложении, остров покажет, когда kl!ck откроют. По той же причине
/// задержка в острове — с последнего замера в приложении; время система отсчитывает сама.
@MainActor
enum WidgetBridge {
    private static var last: WidgetState?

    static func update(_ st: StateView, name: String?, ping: Int?, flag: String?) {
        let since = st.since.flatMap { $0 > 0 ? $0 : nil }
        let state = WidgetState(vpn: st.vpn, name: name, server: st.server, since: since, ping: ping, flag: flag)
        guard state != last else { return }
        last = state
        state.save()
        WidgetCenter.shared.reloadAllTimelines()
        if #available(iOS 18.0, *) { ControlCenter.shared.reloadAllControls() }
        if #available(iOS 16.2, *) { syncActivity(state) }
    }

    /// Поменяли остров в настройках — показать, убрать или перерисовать сразу, не дожидаясь смены состояния.
    static func restyle() {
        guard let last else { return }
        if #available(iOS 16.2, *) { syncActivity(last) }
    }

    @available(iOS 16.2, *)
    private static func syncActivity(_ state: WidgetState) {
        let trouble = ["reconnecting", "server_down"].contains(state.vpn)
        let show: Bool
        switch Prefs.islandMode {
        case "always": show = trouble || state.vpn == "connected"
        case "trouble": show = trouble
        default: show = false
        }
        let current = Activity<KlickActivityAttributes>.activities
        guard show else {
            for a in current { Task { await a.end(nil, dismissalPolicy: .immediate) } }
            return
        }
        let since = state.since.map { Date(timeIntervalSince1970: $0) } ?? .now
        let content = ActivityContent(state: KlickActivityAttributes.ContentState(vpn: state.vpn, server: state.server, since: since,
                                                                                  left: Prefs.left, right: Prefs.right,
                                                                                  ping: state.ping, flag: state.flag),
                                      staleDate: nil)
        if let a = current.first {
            Task { await a.update(content) }
        } else if ActivityAuthorizationInfo().areActivitiesEnabled {
            _ = try? Activity.request(attributes: KlickActivityAttributes(name: state.name ?? "kl!ck"), content: content)
        }
    }
}
