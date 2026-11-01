import AppKit
import SwiftUI

@main
struct KlickMacApp: App {
    @StateObject private var engine = Engine.shared

    var body: some Scene {
        // Окно фиксированного размера, как в прототипе; заголовок прозрачный, «светофоры» поверх.
        Window("kl!ck β", id: "main") {
            RootView()
                .environmentObject(engine)
                .frame(width: 380, height: 720)
                .onAppear { engine.start() }
        }
        .windowStyle(.hiddenTitleBar)
        .windowResizability(.contentSize)

        // Значок в строке меню: вкл/выкл и сервер — не открывая окно.
        MenuBarExtra {
            MenuBarContent().environmentObject(engine)
        } label: {
            Image(systemName: engine.state?.vpn == "connected" ? "power.circle.fill" : "power.circle")
        }
    }
}

private struct MenuBarContent: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.openWindow) private var openWindow

    var body: some View {
        Text(Texts.status(engine.state?.vpn))
        if let server = engine.state?.server, engine.isRunning { Text(server) }
        Divider()
        Button(engine.isRunning ? "Отключить VPN" : "Включить VPN") {
            Task { await engine.power(viewing: nil) }
        }
        .disabled(!engine.up || engine.pending != nil)
        Button("Открыть kl!ck β") {
            openWindow(id: "main")
            NSApp.activate(ignoringOtherApps: true)
        }
        Divider()
        Button("Выйти") { NSApp.terminate(nil) }
    }
}

enum Tab: String, CaseIterable {
    case home, routes, settings

    var icon: String {
        switch self {
        case .home: "house"
        case .routes: "arrow.triangle.branch"
        case .settings: "gearshape"
        }
    }

    var title: String {
        switch self {
        case .home: "Главная"
        case .routes: "Маршруты"
        case .settings: "Настройки"
        }
    }
}

struct RootView: View {
    @EnvironmentObject var engine: Engine
    // Снимки в CI: вкладка из окружения (`KLICK_TAB=routes`) — кликать на раннере нечем.
    @State private var tab: Tab = Tab(rawValue: ProcessInfo.processInfo.environment["KLICK_TAB"] ?? "") ?? .home

    private var scheme: ColorScheme? {
        switch engine.settings?.appearance?.theme {
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }

    var body: some View {
        ZStack(alignment: .bottom) {
            K.bg.ignoresSafeArea()
            Group {
                switch tab {
                case .home: HomeView(onTab: { tab = $0 })
                case .routes: RoutesView()
                case .settings: SettingsView()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
            .transition(.opacity)

            // Затухание под панелью, чтобы список не обрывался резко.
            LinearGradient(colors: [K.bg.opacity(0), K.bg], startPoint: .top, endPoint: .bottom)
                .frame(height: 70)
                .allowsHitTesting(false)

            VStack(spacing: 10) {
                ToastView()
                TabBar(tab: $tab)
            }
            .padding(.bottom, 16)

            if !engine.up && !engine.mock { OfflineCard() }
        }
        .ignoresSafeArea(edges: .top)
        .preferredColorScheme(scheme)
        .onChange(of: engine.settings?.appearance?.accent) { _, v in
            if let v, let hex = K.parseHex(v) { K.accentHex = hex }
        }
    }
}

/// Три раздела в стеклянной капсуле; выбранный — подложкой.
struct TabBar: View {
    @Binding var tab: Tab
    @Namespace private var ns

    var body: some View {
        HStack(spacing: 2) {
            ForEach(Tab.allCases, id: \.self) { t in
                Button {
                    withAnimation(.spring(response: 0.32, dampingFraction: 0.8)) { tab = t }
                } label: {
                    Image(systemName: t.icon)
                        .font(.system(size: 17, weight: .medium))
                        .foregroundStyle(tab == t ? K.text : K.text2)
                        .frame(width: 54, height: 40)
                        .background {
                            if tab == t { Capsule().fill(K.text.opacity(0.08)).matchedGeometryEffect(id: "tab", in: ns) }
                        }
                }
                .buttonStyle(PressStyle())
                .help(t.title)
            }
        }
        .padding(4)
        .glass(Capsule(), interactive: true)
        .shadow(color: .black.opacity(0.12), radius: 14, y: 6)
    }
}

/// Уведомление над панелью: точка цвета события, заголовок, подпись; само прячется через 3.5 с.
struct ToastView: View {
    @EnvironmentObject var engine: Engine

    var body: some View {
        if let t = engine.toast {
            HStack(spacing: 10) {
                Circle().fill(t.tone == "bad" ? K.red : t.tone == "warn" ? K.orange : t.tone == "dim" ? K.text2 : K.ok).frame(width: 8, height: 8)
                VStack(alignment: .leading, spacing: 1) {
                    Text(t.title).font(.system(size: 13, weight: .semibold)).foregroundStyle(K.text)
                    if let sub = t.sub { Text(sub).font(.system(size: 12)).foregroundStyle(K.text2).lineLimit(1) }
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .glass(RoundedRectangle(cornerRadius: 18, style: .continuous))
            .padding(.horizontal, 16)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .id(t.id)
            .task(id: t.id) {
                try? await Task.sleep(for: .seconds(3.5))
                if engine.toast?.id == t.id { withAnimation { engine.toast = nil } }
            }
            .onTapGesture { withAnimation { engine.toast = nil } }
        }
    }
}

/// Служба не отвечает: окно без неё ничего не умеет.
struct OfflineCard: View {
    var body: some View {
        VStack(spacing: 10) {
            Image(systemName: "bolt.horizontal.circle").font(.system(size: 30)).foregroundStyle(K.orange)
            Text("Служба kl!ck не отвечает").font(.system(size: 15, weight: .semibold)).foregroundStyle(K.text)
            Text("Это окно — только интерфейс: VPN держит служба из обычного kl!ck.pkg. Установите kl!ck или запустите его — окно подключится само.")
                .font(.system(size: 12.5)).foregroundStyle(K.text2).multilineTextAlignment(.center)
        }
        .padding(20)
        .card(22)
        .padding(.horizontal, 22)
        .frame(maxHeight: .infinity)
        .background(K.bg.opacity(0.6))
    }
}
