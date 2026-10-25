import SwiftUI

extension Color {
    /// Акцент kl!ck — тот же зелёный, что у веб-версии.
    static let klickAccent = Color(red: 0.19, green: 0.82, blue: 0.35)
}

extension View {
    /// iOS 26 и Xcode 26: стекло Liquid Glass вокруг круглой кнопки; раньше — как было.
    @ViewBuilder
    func glassCircle() -> some View {
        #if compiler(>=6.2)
        if #available(iOS 26.0, *) {
            self.glassEffect(.regular.interactive(), in: .circle)
        } else {
            self
        }
        #else
        self
        #endif
    }
}

@main
struct KlickApp: App {
    @StateObject private var engine = Engine.shared
    @AppStorage("theme") private var theme = "system"

    var body: some Scene {
        WindowGroup {
            RootView()
                .environmentObject(engine)
                .tint(.klickAccent)
                .preferredColorScheme(theme == "light" ? .light : theme == "dark" ? .dark : nil)
                .onAppear { engine.start() }
        }
    }
}

struct RootView: View {
    @EnvironmentObject var engine: Engine
    /// Вкладка при запуске: `KLICK_TAB` — для снимков экрана в симуляторе (scripts/ios/native-sim-test.sh).
    @State private var tab = Int(ProcessInfo.processInfo.environment["KLICK_TAB"] ?? "") ?? 0

    var body: some View {
        Group {
            if let error = engine.startError {
                VStack(spacing: 12) {
                    Image(systemName: "exclamationmark.triangle").font(.largeTitle).foregroundColor(.orange)
                    Text("kl!ck не запустился").font(.headline)
                    Text(error).font(.footnote).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                .padding()
            } else if !engine.up {
                ProgressView("Запускаю kl!ck…")
            } else {
                TabView(selection: $tab) {
                    HomeView()
                        .tabItem { Label("Главная", systemImage: "power") }
                        .tag(0)
                    AddView(tab: $tab)
                        .tabItem { Label("Добавить", systemImage: "plus.circle") }
                        .tag(1)
                    SettingsView()
                        .tabItem { Label("Настройки", systemImage: "gearshape") }
                        .tag(2)
                }
            }
        }
        .overlay(alignment: .top) { ToastView() }
    }
}

struct ToastView: View {
    @EnvironmentObject var engine: Engine

    var body: some View {
        if let toast = engine.toast {
            HStack(spacing: 8) {
                Circle().fill(toast.bad ? Color.orange : Color.klickAccent).frame(width: 8, height: 8)
                Text(toast.text).font(.subheadline.weight(.medium))
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(.regularMaterial, in: Capsule())
            .shadow(color: .black.opacity(0.15), radius: 10, y: 4)
            .padding(.top, 8)
            .transition(.move(edge: .top).combined(with: .opacity))
            .onTapGesture { engine.toast = nil }
            .task(id: toast.id) {
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                if engine.toast?.id == toast.id { withAnimation { engine.toast = nil } }
            }
        }
    }
}
