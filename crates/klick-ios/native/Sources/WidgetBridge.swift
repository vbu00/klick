import ActivityKit
import Foundation
import WidgetKit

/// Приложение → виджеты: состояние для виджета и Пункта управления, Live Activity, пока VPN работает.
/// Live Activity iOS разрешает начать только из открытого приложения: включили VPN из Пункта управления —
/// она появится, когда kl!ck откроют.
@MainActor
enum WidgetBridge {
    private static var last: WidgetState?

    static func update(_ st: StateView, name: String?) {
        let since = st.since.flatMap { $0 > 0 ? $0 : nil }
        let state = WidgetState(vpn: st.vpn, name: name, server: st.server, since: since)
        guard state != last else { return }
        last = state
        state.save()
        WidgetCenter.shared.reloadAllTimelines()
        if #available(iOS 18.0, *) { ControlCenter.shared.reloadAllControls() }
        if #available(iOS 16.2, *) { syncActivity(state) }
    }

    @available(iOS 16.2, *)
    private static func syncActivity(_ state: WidgetState) {
        let live = ["connected", "reconnecting", "server_down"].contains(state.vpn)
        let current = Activity<KlickActivityAttributes>.activities
        guard live, let since = state.since else {
            for a in current { Task { await a.end(nil, dismissalPolicy: .immediate) } }
            return
        }
        let content = ActivityContent(state: KlickActivityAttributes.ContentState(vpn: state.vpn, server: state.server, since: Date(timeIntervalSince1970: since)),
                                      staleDate: nil)
        if let a = current.first {
            Task { await a.update(content) }
        } else if ActivityAuthorizationInfo().areActivitiesEnabled {
            _ = try? Activity.request(attributes: KlickActivityAttributes(name: state.name ?? "kl!ck"), content: content)
        }
    }
}
