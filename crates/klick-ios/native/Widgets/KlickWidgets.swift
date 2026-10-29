import ActivityKit
import AppIntents
import SwiftUI
import WidgetKit

// Виджеты kl!ck β (iOS 18+): на рабочем столе и экране блокировки, Live Activity с Dynamic Island,
// кнопка в Пункте управления. VPN включается и выключается без приложения (VPNSwitch).

@main
struct KlickWidgets: WidgetBundle {
    var body: some Widget {
        StatusWidget()
        KlickLiveActivity()
        VPNControl()
    }
}

private let accent = Color(red: 0x30 / 255, green: 0xd1 / 255, blue: 0x58 / 255)

// MARK: Виджет

struct StatusEntry: TimelineEntry {
    let date: Date
    let on: Bool
    let state: WidgetState?
}

struct StatusProvider: TimelineProvider {
    func placeholder(in context: Context) -> StatusEntry {
        StatusEntry(date: .now, on: true, state: WidgetState(vpn: "connected", name: "Подписка", server: "Hysteria 2", since: Date.now.timeIntervalSince1970 - 1800))
    }

    func getSnapshot(in context: Context, completion: @escaping (StatusEntry) -> Void) {
        Task { completion(await entry()) }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<StatusEntry>) -> Void) {
        Task { completion(Timeline(entries: [await entry()], policy: .after(.now.addingTimeInterval(15 * 60)))) }
    }

    /// Включён ли VPN — у самой iOS (точно), сервер и время — из того, что записало приложение.
    private func entry() async -> StatusEntry {
        StatusEntry(date: .now, on: await VPNSwitch.isOn(), state: WidgetState.load())
    }
}

struct StatusWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "app.klick.native.status", provider: StatusProvider()) { entry in
            StatusView(entry: entry)
                .containerBackground(for: .widget) { Color(red: 0x1a / 255, green: 0x1a / 255, blue: 0x1d / 255) }
        }
        .configurationDisplayName("kl!ck")
        .description("Состояние VPN и кнопка включения.")
        .supportedFamilies([.systemSmall, .systemMedium, .accessoryCircular, .accessoryRectangular])
    }
}

struct StatusView: View {
    @Environment(\.widgetFamily) private var family
    let entry: StatusEntry

    private var server: String { entry.state?.server ?? entry.state?.name ?? "kl!ck" }
    private var since: Date? { entry.on ? entry.state?.since.map { Date(timeIntervalSince1970: $0) } : nil }

    var body: some View {
        switch family {
        case .accessoryCircular:
            Button(intent: ToggleVPNIntent(value: !entry.on)) {
                ZStack {
                    AccessoryWidgetBackground()
                    Image(systemName: "power").font(.system(size: 20, weight: .semibold))
                }
            }
            .buttonStyle(.plain)
            .widgetAccentable()
        case .accessoryRectangular:
            VStack(alignment: .leading, spacing: 1) {
                Text(entry.on ? "kl!ck · включён" : "kl!ck · выключен").font(.headline).widgetAccentable()
                Text(server).font(.caption).lineLimit(1)
                if let since { Text(since, style: .timer).font(.caption).monospacedDigit() }
            }
        case .systemMedium:
            HStack(spacing: 16) {
                power(size: 64)
                VStack(alignment: .leading, spacing: 4) {
                    title
                    Text(server).font(.system(size: 15, weight: .semibold)).foregroundStyle(.white).lineLimit(1)
                    if let since {
                        Text(since, style: .timer).font(.system(size: 22, weight: .medium)).monospacedDigit().foregroundStyle(.white)
                    } else {
                        Text("Нажмите, чтобы включить").font(.system(size: 13)).foregroundStyle(.white.opacity(0.55))
                    }
                }
                Spacer(minLength: 0)
            }
        default:
            VStack(alignment: .leading, spacing: 0) {
                title
                Spacer(minLength: 6)
                power(size: 56)
                Spacer(minLength: 6)
                Text(entry.on ? server : "выключен").font(.system(size: 12, weight: .medium)).foregroundStyle(.white.opacity(0.75)).lineLimit(1)
            }
        }
    }

    private var title: some View {
        HStack(spacing: 6) {
            Circle().fill(entry.on ? accent : Color.gray).frame(width: 7, height: 7)
            Text("kl!ck").font(.system(size: 13, weight: .bold)).foregroundStyle(.white)
        }
    }

    /// Кнопка питания: нажатие включает или выключает VPN, не открывая приложение.
    private func power(size: CGFloat) -> some View {
        Button(intent: ToggleVPNIntent(value: !entry.on)) {
            Image(systemName: "power")
                .font(.system(size: size * 0.4, weight: .semibold))
                .foregroundStyle(entry.on ? Color(red: 0.04, green: 0.04, blue: 0.05) : accent)
                .frame(width: size, height: size)
                .background(Circle().fill(entry.on ? accent : Color.white.opacity(0.1)))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(entry.on ? "Выключить VPN" : "Включить VPN")
    }
}

// MARK: Live Activity и Dynamic Island

/// Остров — как выбрано в настройках kl!ck: только при беде со связью или всё время с VPN. Свёрнутый —
/// Island.leading / trailing (что слева и справа, выбирают там же); раскрытый и экран блокировки —
/// состояние, сервер, задержка и «Отключить».
struct KlickLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: KlickActivityAttributes.self) { context in
            // Экран блокировки.
            HStack(spacing: 12) {
                Image(systemName: symbol(context.state.vpn)).font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(Island.tint(context.state.vpn))
                    .frame(width: 40, height: 40).background(Circle().fill(.white.opacity(0.1)))
                VStack(alignment: .leading, spacing: 2) {
                    status(context.state).font(.system(size: 13)).foregroundStyle(.white.opacity(0.6))
                    Text(title(context)).font(.system(size: 15, weight: .semibold)).foregroundStyle(.white).lineLimit(1)
                }
                Spacer(minLength: 0)
                off
            }
            .padding(16)
            .activityBackgroundTint(Color(red: 0x1a / 255, green: 0x1a / 255, blue: 0x1d / 255).opacity(0.9))
            .activitySystemActionForegroundColor(.white)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    Label { Text("kl!ck") } icon: { Image(systemName: symbol(context.state.vpn)).foregroundStyle(Island.tint(context.state.vpn)) }
                        .font(.system(size: 14, weight: .semibold))
                }
                DynamicIslandExpandedRegion(.trailing) {
                    if let ping = context.state.ping, !Island.trouble(context.state.vpn) {
                        Text("\(ping) мс").font(.system(size: 14, weight: .semibold)).monospacedDigit().foregroundStyle(Island.green)
                    }
                }
                DynamicIslandExpandedRegion(.bottom) {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            status(context.state).font(.system(size: 12)).foregroundStyle(.secondary)
                            Text(title(context)).font(.system(size: 15, weight: .semibold)).lineLimit(1)
                        }
                        Spacer()
                        off
                    }
                }
            } compactLeading: {
                Island.leading(vpn: context.state.vpn, left: context.state.left, flag: context.state.flag)
            } compactTrailing: {
                Island.trailing(vpn: context.state.vpn, right: context.state.right, server: context.state.server,
                                ping: context.state.ping, since: context.state.since)
            } minimal: {
                Island.leading(vpn: context.state.vpn, left: context.state.left, flag: context.state.flag)
            }
            .keylineTint(Island.tint(context.state.vpn))
        }
    }

    private func symbol(_ vpn: String) -> String {
        Island.trouble(vpn) ? Island.icon(vpn) : "checkmark.shield.fill"
    }

    /// «сервер не отвечает» или «подключено · 0:42:05».
    @ViewBuilder
    private func status(_ s: KlickActivityAttributes.ContentState) -> some View {
        if Island.trouble(s.vpn) {
            Text(Island.status(s.vpn))
        } else {
            HStack(spacing: 0) {
                Text("подключено · ")
                Text(timerInterval: s.since...Date.distantFuture, countsDown: false).monospacedDigit()
            }
        }
    }

    /// Сервер с флагом страны, если она есть в имени.
    private func title(_ context: ActivityViewContext<KlickActivityAttributes>) -> String {
        (context.state.flag.map { WidgetState.emoji($0) + " " } ?? "") + (context.state.server ?? context.attributes.name)
    }

    private var off: some View {
        Button(intent: ToggleVPNIntent(value: false)) {
            Text("Отключить").font(.system(size: 13, weight: .semibold))
        }
        .tint(.red)
    }
}

// MARK: Пункт управления

struct VPNControl: ControlWidget {
    var body: some ControlWidgetConfiguration {
        StaticControlConfiguration(kind: "app.klick.native.vpn", provider: Provider()) { on in
            ControlWidgetToggle("kl!ck", isOn: on, action: ToggleVPNIntent()) { isOn in
                Label(isOn ? "Подключено" : "Выключено", systemImage: "power")
            }
            .tint(accent)
        }
        .displayName("kl!ck VPN")
        .description("Включить и выключить VPN, не открывая приложение.")
    }

    struct Provider: ControlValueProvider {
        var previewValue: Bool { false }

        func currentValue() async throws -> Bool { await VPNSwitch.isOn() }
    }
}
