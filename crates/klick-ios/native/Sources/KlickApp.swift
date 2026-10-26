import SwiftUI

@main
struct KlickApp: App {
    @StateObject private var engine = Engine.shared
    /// Как у веб-версии: по умолчанию тёмная («Графит»).
    @AppStorage("theme") private var theme = "dark"
    @AppStorage("glass") private var glass = true

    /// Тема; `KLICK_THEME` — для снимков в симуляторе (scripts/ios/native-sim-test.sh).
    private var scheme: ColorScheme? {
        switch ProcessInfo.processInfo.environment["KLICK_THEME"].flatMap({ $0.isEmpty ? nil : $0 }) ?? theme {
        case "light": return .light
        case "dark": return .dark
        default: return nil
        }
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(engine)
                .environment(\.klickGlass, glass)
                .tint(K.accent)
                .preferredColorScheme(scheme)
                .onAppear { engine.start() }
        }
    }
}

/// Вкладки веб-версии (Chrome.tsx: TABS).
enum Tab: Int {
    case home, connection, add, settings
}

struct RootView: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.klickGlass) private var glass
    /// Вкладка при запуске: `KLICK_TAB` — для снимков экрана в симуляторе (scripts/ios/native-sim-test.sh).
    @State private var tab = Int(ProcessInfo.processInfo.environment["KLICK_TAB"] ?? "") ?? 0

    var body: some View {
        ZStack {
            KBackground()
            VStack(spacing: 0) {
                TitleBar()
                if let error = engine.startError {
                    VStack(spacing: 8) {
                        Text("Движок kl!ck не запустился").font(.system(size: 15, weight: .semibold)).foregroundColor(K.text)
                        Text(error).font(.system(size: 12)).foregroundColor(K.dim).multilineTextAlignment(.center)
                    }
                    .padding(24)
                    .frame(maxHeight: .infinity)
                } else if !engine.up {
                    ProgressView().tint(K.dim).frame(maxHeight: .infinity)
                } else {
                    Group {
                        switch Tab(rawValue: tab) ?? .home {
                        case .home: HomeView(tab: $tab)
                        case .connection: ConnectionScreen()
                        case .add: AddView(tab: $tab)
                        case .settings: SettingsView()
                        }
                    }
                    .frame(maxHeight: .infinity)
                    .overlay(alignment: .bottom) { bottomBar }
                }
            }
        }
    }

    /// `.nav` и `.toasts`: панель вкладок в 22 от низа, уведомления над ней. Без стекла под панелью —
    /// затухание содержимого (`.nav-fade`), со стеклом оно не нужно.
    private var bottomBar: some View {
        VStack(spacing: 8) {
            ToastView()
            KTabBar(tab: $tab, items: [("home", "Главная"), ("rules", "Соединение"), ("plus", "Добавить"), ("settings", "Настройки")])
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 8)
        .frame(maxWidth: .infinity)
        .background(alignment: .bottom) {
            if !glass {
                LinearGradient(stops: [.init(color: K.win.opacity(0), location: 0), .init(color: K.win.opacity(0.85), location: 0.45), .init(color: K.win, location: 1)],
                               startPoint: .top, endPoint: .bottom)
                    .frame(height: 120)
                    .ignoresSafeArea(edges: .bottom)
                    .allowsHitTesting(false)
            }
        }
    }
}

/// `.titlebar`: значок состояния и «kl!ck», 44, линия снизу (на iOS — без кнопок окна).
struct TitleBar: View {
    @EnvironmentObject var engine: Engine

    private var icon: String {
        if engine.startError != nil { return "status-error" }
        switch engine.state?.vpn {
        case "connected": return "status-success"
        case "connecting", "reconnecting", "server_down": return "status-warning"
        case "error": return "status-error"
        default: return engine.state != nil && engine.state?.connection == nil ? "status-no-connection" : "status-default"
        }
    }

    var body: some View {
        HStack(spacing: 6) {
            Image(icon).resizable().scaledToFit().frame(width: 18, height: 18)
                .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
            Text("kl!ck").font(.system(size: 14, weight: .bold)).foregroundColor(K.text).padding(.leading, 2)
            Spacer()
        }
        .padding(.leading, 16)
        .padding(.trailing, 10)
        .frame(height: 44)
        .overlay(alignment: .bottom) { Rectangle().fill(K.line).frame(height: 1) }
    }
}

/// `.toast`: светлая плашка на тёмной теме (фон — цвет текста), точка цвета события, заголовок и подпись.
struct ToastView: View {
    @EnvironmentObject var engine: Engine

    var body: some View {
        if let toast = engine.toast {
            HStack(spacing: 10) {
                Circle().fill(color(toast.tone)).frame(width: 8, height: 8).padding(.leading, 6).padding(.trailing, 2)
                VStack(alignment: .leading, spacing: 1) {
                    Text(toast.title).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                    if let sub = toast.sub, !sub.isEmpty {
                        Text(sub).font(.system(size: 12)).opacity(0.65).lineLimit(1)
                    }
                }
                .padding(.trailing, 10)
                Spacer(minLength: 0)
            }
            .foregroundColor(K.win)
            .padding(.vertical, 6)
            .padding(.leading, 8)
            .frame(minHeight: 48)
            .background(K.text, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            .shadow(color: .black.opacity(0.24), radius: 12, y: 8)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .onTapGesture { engine.toast = nil }
            .task(id: toast.id) {
                try? await Task.sleep(nanoseconds: 3_500_000_000)
                if engine.toast?.id == toast.id { withAnimation { engine.toast = nil } }
            }
        }
    }

    private func color(_ tone: String) -> Color {
        switch tone {
        case "bad": return K.red
        case "warn": return K.orange
        case "dim": return K.dim
        default: return K.accent
        }
    }
}
