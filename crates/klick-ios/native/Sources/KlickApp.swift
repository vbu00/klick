import SwiftUI

@main
struct KlickApp: App {
    @StateObject private var engine = Engine.shared
    /// Как у веб-версии: по умолчанию тёмная («Графит»).
    @AppStorage("theme") private var theme = "dark"
    @AppStorage("glass") private var glass = true

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(engine)
                .environment(\.klickGlass, glass)
                .tint(K.accent)
                .preferredColorScheme(theme == "light" ? .light : theme == "dark" ? .dark : nil)
                .onAppear { engine.start() }
        }
    }
}

struct RootView: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.klickGlass) private var glass
    /// Вкладка при запуске: `KLICK_TAB` — для снимков экрана в симуляторе (scripts/ios/native-sim-test.sh).
    @State private var tab = Int(ProcessInfo.processInfo.environment["KLICK_TAB"] ?? "") ?? 0

    var body: some View {
        ZStack {
            KBackground()
            if let error = engine.startError {
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle").font(.largeTitle).foregroundColor(K.orange)
                    Text("kl!ck не запустился").font(.headline).foregroundColor(K.text)
                    Text(error).font(.footnote).foregroundColor(K.dim).multilineTextAlignment(.center)
                }
                .padding()
            } else if !engine.up {
                ProgressView().tint(K.dim)
            } else {
                Group {
                    switch tab {
                    case 1: AddView(tab: $tab)
                    case 2: SettingsView()
                    default: HomeView()
                    }
                }
                .overlay(alignment: .bottom) { bottomBar }
            }
        }
    }

    /// Панель вкладок и уведомление над ней. Без стекла под панелью — затухание содержимого, со стеклом
    /// оно не нужно: под стеклом и так видно, что под ним (как в веб-версии).
    private var bottomBar: some View {
        VStack(spacing: 10) {
            ToastView()
            KTabBar(tab: $tab, items: [("power", "Главная"), ("plus", "Добавить"), ("gearshape.fill", "Настройки")])
        }
        .padding(.bottom, 8)
        .frame(maxWidth: .infinity)
        .background(alignment: .bottom) {
            if !glass {
                LinearGradient(colors: [K.win.opacity(0), K.win.opacity(0.85), K.win], startPoint: .top, endPoint: .bottom)
                    .frame(height: 110)
                    .ignoresSafeArea(edges: .bottom)
                    .allowsHitTesting(false)
            }
        }
    }
}

struct ToastView: View {
    @EnvironmentObject var engine: Engine

    var body: some View {
        if let toast = engine.toast {
            HStack(spacing: 8) {
                Circle().fill(toast.bad ? K.orange : K.accent).frame(width: 7, height: 7)
                Text(toast.text).font(.system(size: 13, weight: .medium)).foregroundColor(K.text)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 11)
            .kSurface(Capsule(), solid: K.elem)
            .padding(.horizontal, 24)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .onTapGesture { engine.toast = nil }
            .task(id: toast.id) {
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                if engine.toast?.id == toast.id { withAnimation { engine.toast = nil } }
            }
        }
    }
}
