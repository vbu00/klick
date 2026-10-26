import SwiftUI

@main
struct KlickApp: App {
    @StateObject private var engine = Engine.shared
    @StateObject private var ui = UIState()
    /// Копия темы из движка: при запуске приложение сразу в своих цветах, пока движок не ответил.
    @AppStorage("appearance") private var cached = ""

    private var appearance: Appearance {
        var a = engine.settings?.appearance ?? (try? JSONDecoder().decode(Appearance.self, from: Data(cached.utf8))) ?? .standard
        // KLICK_THEME и KLICK_GLASS — для снимков в симуляторе (scripts/ios/native-sim-test.sh).
        let env = ProcessInfo.processInfo.environment
        if let t = env["KLICK_THEME"], !t.isEmpty { a.theme = t }
        if let g = env["KLICK_GLASS"], !g.isEmpty { a.glass = g == "1" }
        return a
    }

    var body: some Scene {
        WindowGroup {
            let a = appearance
            let _ = Palette.apply(theme: a.theme, base: a.base, accent: a.accent, glass: a.glass)
            // Цвета темы — динамические UIColor: сменилась тема — экраны собираются заново (вкладка и
            // подэкран живут в UIState и не сбрасываются).
            RootView()
                .id(Palette.key)
                .environmentObject(engine)
                .environmentObject(ui)
                .environment(\.klickGlass, a.glass)
                .tint(K.accent)
                .preferredColorScheme(Palette.scheme)
                .onAppear { engine.start() }
                .onChange(of: engine.settings?.appearance) { a in
                    if let a, let d = try? JSONEncoder().encode(a) { cached = String(decoding: d, as: UTF8.self) }
                }
        }
    }
}

/// Что открыто: вкладка и подэкран настроек — переживают пересборку экранов при смене темы.
final class UIState: ObservableObject {
    /// Вкладка при запуске: `KLICK_TAB` — для снимков экрана в симуляторе.
    @Published var tab = Int(ProcessInfo.processInfo.environment["KLICK_TAB"] ?? "") ?? 0
    /// Подэкран настроек; `KLICK_SETTINGS_SUB` — для снимков в симуляторе.
    @Published var settingsSub = ProcessInfo.processInfo.environment["KLICK_SETTINGS_SUB"].flatMap { $0.isEmpty ? nil : $0 }
}

/// Вкладки веб-версии (Chrome.tsx: TABS).
enum Tab: Int {
    case home, connection, add, settings
}

struct RootView: View {
    @EnvironmentObject var engine: Engine
    @EnvironmentObject var ui: UIState
    @Environment(\.klickGlass) private var glass
    private var tab: Binding<Int> { $ui.tab }

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
                } else if let native = nativeTabs {
                    native
                } else {
                    Group {
                        switch Tab(rawValue: ui.tab) ?? .home {
                        case .home: HomeView(tab: tab)
                        case .connection: ConnectionScreen()
                        case .add: AddView(tab: tab)
                        case .settings: SettingsView()
                        }
                    }
                    .frame(maxHeight: .infinity)
                    .overlay(alignment: .bottom) { bottomBar }
                }
            }
        }
    }

    /// Со стеклом на iOS 26 — системная панель вкладок: плавающая, из Liquid Glass, выделение перетекает
    /// между вкладками. Значки — те же, что у веб-версии.
    private var nativeTabs: AnyView? {
        #if compiler(>=6.2)
        if glass, #available(iOS 26.0, *) {
            return AnyView(
                TabView(selection: tab) {
                    SwiftUI.Tab("Главная", image: "home", value: Tab.home.rawValue) { HomeView(tab: tab).background(KBackground()) }
                    SwiftUI.Tab("Соединение", image: "rules", value: Tab.connection.rawValue) { ConnectionScreen().background(KBackground()) }
                    SwiftUI.Tab("Добавить", image: "plus", value: Tab.add.rawValue) { AddView(tab: tab).background(KBackground()) }
                    SwiftUI.Tab("Настройки", image: "settings", value: Tab.settings.rawValue) { SettingsView().background(KBackground()) }
                }
                .tint(K.accent)
                .tabBarMinimizeBehavior(.onScrollDown)
                .overlay(alignment: .bottom) { ToastView().padding(.horizontal, 16).padding(.bottom, 96) }
            )
        }
        #endif
        return nil
    }

    /// `.nav` и `.toasts`: панель вкладок в 22 от низа, уведомления над ней. Без стекла под панелью —
    /// затухание содержимого (`.nav-fade`), со стеклом оно не нужно.
    private var bottomBar: some View {
        VStack(spacing: 8) {
            ToastView()
            KTabBar(tab: tab, items: [("home", "Главная"), ("rules", "Соединение"), ("plus", "Добавить"), ("settings", "Настройки")])
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
    @Environment(\.klickGlass) private var glass

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
            .foregroundColor(glass ? K.text : K.win)
            .padding(.vertical, 6)
            .padding(.leading, 8)
            .frame(minHeight: 48)
            .kSurface(RoundedRectangle(cornerRadius: 14, style: .continuous), solid: K.text, interactive: true)
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
