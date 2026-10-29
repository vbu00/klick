import SwiftUI

/// Свёрнутый Dynamic Island kl!ck — один и тот же в острове (виджеты) и в предпросмотре в настройках.
/// Беда со связью — оранжевый знак и «связь…» / «нет связи»: выбор слева и справа тогда не действует.
@available(iOS 16.0, *)
enum Island {
    static let green = Color(red: 0x30 / 255, green: 0xd1 / 255, blue: 0x58 / 255)

    static func trouble(_ vpn: String) -> Bool { vpn == "reconnecting" || vpn == "server_down" }

    static func icon(_ vpn: String) -> String {
        vpn == "reconnecting" ? "arrow.triangle.2.circlepath" : "exclamationmark.triangle.fill"
    }

    static func status(_ vpn: String) -> String {
        switch vpn {
        case "reconnecting": return "переподключаюсь…"
        case "server_down": return "сервер не отвечает"
        default: return "подключено"
        }
    }

    static func tint(_ vpn: String) -> Color { trouble(vpn) ? .orange : green }

    /// Слева: логотип, флаг страны сервера (страны в имени нет — логотип) или точка.
    @ViewBuilder
    static func leading(vpn: String, left: String, flag: String?) -> some View {
        if trouble(vpn) {
            Image(systemName: icon(vpn)).font(.system(size: 13, weight: .bold)).foregroundStyle(.orange)
        } else if left == "flag", let flag {
            Text(WidgetState.emoji(flag)).font(.system(size: 15))
        } else if left == "dot" {
            Circle().fill(green).frame(width: 9, height: 9)
        } else {
            Text("kl!ck").font(.system(size: 13, weight: .heavy)).foregroundStyle(green)
        }
    }

    /// Справа: ничего, задержка, сервер или время с подключения (его система отсчитывает сама).
    @ViewBuilder
    static func trailing(vpn: String, right: String, server: String?, ping: Int?, since: Date) -> some View {
        if trouble(vpn) {
            Text(vpn == "reconnecting" ? "связь…" : "нет связи").font(.system(size: 12, weight: .semibold)).foregroundStyle(.orange)
        } else {
            switch right {
            case "ping":
                Text(ping.map { "\($0) мс" } ?? "— мс").font(.system(size: 12, weight: .semibold)).monospacedDigit()
                    .foregroundStyle(ping == nil ? Color.gray : green)
            case "server":
                Text(server ?? "kl!ck").font(.system(size: 12, weight: .semibold)).foregroundStyle(.white).lineLimit(1).frame(maxWidth: 76)
            case "time":
                Text(timerInterval: since...Date.distantFuture, countsDown: false).font(.system(size: 12, weight: .semibold))
                    .monospacedDigit().foregroundStyle(.white).frame(maxWidth: 56)
            default:
                EmptyView()
            }
        }
    }
}
