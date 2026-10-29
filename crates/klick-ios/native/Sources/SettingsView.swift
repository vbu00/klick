import SwiftUI
import UIKit

/// «Настройки» — как ui/src/screens/Settings.tsx на iOS: группы карточек и подэкраны «Если сервер
/// недоступен», «Тема», «Журнал», «О приложении».
struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @EnvironmentObject var ui: UIState

    var body: some View {
        Group {
            switch ui.settingsSub {
            case "down": ServerDownScreen { ui.settingsSub = nil }
            case "theme": ThemeScreen { ui.settingsSub = nil }
            case "log": LogScreen { ui.settingsSub = nil }
            case "about": AboutScreen { ui.settingsSub = nil }
            default: SettingsMain { ui.settingsSub = $0 }
            }
        }
    }
}

/// Что делать, если сервер не ответил (i18n.ts: serverDown).
let serverDownTexts: [(String, String, String, String?)] = [
    ("reconnect", "Ждать и переподключаться", "Три попытки за 15 секунд, потом уведомление «Сервер не отвечает». Дальше kl!ck проверяет раз в минуту и сам вернёт связь.", "По умолчанию"),
    ("next_working", "Переключиться на следующий рабочий", "Если сервер не ответил, kl!ck перейдёт на следующий сервер подключения, который отвечает.", nil),
    ("fastest", "Всегда самый быстрый", "kl!ck держится сервера с наименьшей задержкой и переходит на другой, когда тот становится быстрее.", nil),
]

private let themeNames = [("system", "Системная"), ("light", "Светлая"), ("dark", "Тёмная"), ("custom", "Своя")]

private struct SettingsMain: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.colorScheme) private var scheme
    @State private var about: About?
    @State private var logCount: Int?
    @State private var lang = false
    @AppStorage(Prefs.liveActivity) private var liveActivity = true
    @AppStorage(Prefs.toasts) private var toasts = true
    let onSub: (String) -> Void

    var body: some View {
        KScreen {
            KTitle("Настройки")
            if let s = engine.settings {
                KSectionHead("Подключение")
                KCardList {
                    KRow("Если сервер недоступен", sub: serverDownTexts.first { $0.0 == (s.onServerDown ?? "reconnect") }?.1, chevron: true) { onSub("down") }
                    KDivider()
                    KRow(title: "Восстанавливать подключение", sub: "Система сама включит VPN после перезагрузки и при сбое", wrap: true) {
                        KToggle(isOn: Binding(get: { s.restoreOnLogon ?? false }, set: { v in Task { await engine.setPrefs(["restore_on_logon": v]) } }))
                    }
                }

                KSectionHead("Уведомления")
                KCardList {
                    KRow(title: "Dynamic Island", sub: "Показывать, когда сервер не отвечает или kl!ck переподключается", wrap: true) {
                        KToggle(isOn: Binding(get: { liveActivity }, set: { v in
                            liveActivity = v
                            if !v { WidgetBridge.endActivities() }
                        }))
                    }
                    KDivider()
                    KRow(title: "Плашки внизу экрана", sub: "«Связь восстановлена», «Подписка обновлена» и другие. Ошибки видно всегда", wrap: true) {
                        KToggle(isOn: $toasts)
                    }
                }

                KSectionHead("Оформление")
                KCardList {
                    KRow("Тема", sub: themeSummary, chevron: true) { onSub("theme") }
                    KDivider()
                    KRow(title: "Серверы по странам", sub: "Сначала страны, внутри — их серверы", wrap: true) {
                        KToggle(isOn: Binding(get: { s.serversByCountry ?? false }, set: { v in Task { await engine.setPrefs(["servers_by_country": v]) } }))
                    }
                }

                KSectionHead("Общие")
                KCardList {
                    KRow(title: "Обновлять подписки", sub: "Как просит панель, иначе раз в 12 часов", wrap: true) {
                        KToggle(isOn: Binding(get: { s.autoUpdate ?? true }, set: { v in Task { await engine.setPrefs(["auto_update": v]) } }))
                    }
                    KDivider()
                    KRow("Язык", sub: "Русский", chevron: true) { lang = true }
                }

                KSectionHead("Диагностика")
                KCardList {
                    KRow("Журнал", sub: logCount.map { Texts.plural($0, ("запись", "записи", "записей")) } ?? "Записи службы", chevron: true) { onSub("log") }
                    KDivider()
                    KRow(title: "Скопировать отчёт", sub: "Версии, режим и журнал — без ссылок и адресов сайтов", wrap: true, action: { Task { await report() } }) {
                        KIcon("copy", size: 16).foregroundColor(K.dim2)
                    }
                    KDivider()
                    KRow(title: "Ядро Mihomo", sub: about?.coreVersion ?? "встроено в приложение") {
                        Circle().fill(K.accent).frame(width: 8, height: 8)
                    }
                }

                KCardList(top: 22) {
                    KRow("О приложении", sub: "Версия \(about?.version ?? "…")", chevron: true) { onSub("about") }
                }
            }
        }
        .task {
            about = try? await engine.get(About.self, "about")
            logCount = await engine.log().count
        }
        .confirmationDialog("Язык", isPresented: $lang, titleVisibility: .visible) {
            Button("Русский") {}
            Button("Готово", role: .cancel) {}
        } message: {
            Text("English появится вместе с переводом интерфейса.")
        }
    }

    private var themeSummary: String {
        let a = engine.settings?.appearance ?? .standard
        let name = themeNames.first { $0.0 == a.theme }?.1 ?? "Системная"
        switch a.theme {
        case "custom": return "\(name) · \(Palette.bases.first { $0.0 == a.base }?.1 ?? "")"
        case "system": return "\(name) · сейчас \(scheme == .dark ? "тёмная" : "светлая")"
        default: return name
        }
    }

    private func report() async {
        if let text = await engine.report() {
            UIPasteboard.general.string = text
            engine.toast = Toast("Отчёт скопирован")
        } else {
            engine.toast = Toast("Служба не отдала отчёт", tone: "bad")
        }
    }
}

// MARK: Подэкраны

/// `.choice`: карточка выбора с кружком, названием и пояснением; выбранная — с обводкой.
struct ChoiceCard: View {
    let title: String
    var desc: String?
    var hint: String?
    let on: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(alignment: .top, spacing: 12) {
                KRadio(on: on).padding(.top, 1)
                VStack(alignment: .leading, spacing: 5) {
                    Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(K.text)
                    if let desc { Text(desc).font(.system(size: 12)).foregroundColor(K.text3).lineSpacing(2).fixedSize(horizontal: false, vertical: true) }
                    if let hint { Text(hint).font(.system(size: 12)).foregroundColor(K.dim2) }
                }
                Spacer(minLength: 0)
            }
            .padding(14)
            .kCard()
            .overlay(RoundedRectangle(cornerRadius: K.r(20), style: .continuous).strokeBorder(on ? K.ring : Color.clear, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}

private struct ServerDownScreen: View {
    @EnvironmentObject var engine: Engine
    let onBack: () -> Void

    var body: some View {
        let current = engine.settings?.onServerDown ?? "reconnect"
        KScreen {
            BackButton(label: "Настройки", action: onBack)
            KTitle("Если сервер недоступен")
            VStack(spacing: 8) {
                ForEach(serverDownTexts, id: \.0) { k, title, desc, hint in
                    ChoiceCard(title: title, desc: desc, hint: hint, on: current == k) {
                        if current != k { Task { await engine.setPrefs(["on_server_down": k]) } }
                    }
                }
            }
            .padding(.top, 16)
        }
    }
}

/// «Оформление» — как ThemeScreen в Settings.tsx: тема карточками с превью, стекло, для своей темы — основа
/// и акцент.
private struct ThemeScreen: View {
    @EnvironmentObject var engine: Engine
    let onBack: () -> Void

    private let desc = [
        "system": "Повторяет тему iOS и переключается вместе с ней.",
        "light": "Светлый фон и тёмный текст — удобно при ярком освещении.",
        "dark": "Тёмный фон — меньше нагрузки на глаза вечером.",
        "custom": "Выберите основу и цвет акцента ниже.",
    ]

    var body: some View {
        let a = engine.settings?.appearance ?? .standard
        KScreen {
            BackButton(label: "Настройки", action: onBack)
            Text("Оформление").font(.system(size: 24, weight: .semibold)).foregroundColor(K.text).padding(.top, 12)
            LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
                ForEach(themeNames, id: \.0) { k, label in
                    ThemeCard(kind: k, label: label, a: a) { set(a, theme: k) }
                }
            }
            .padding(.top, 16)
            Text(desc[a.theme] ?? "").font(.system(size: 12)).foregroundColor(K.dim).padding(.horizontal, 4).padding(.top, 10)

            KSectionHead("Материал")
            KCardList {
                KRow(title: "Стекло", sub: "Системное стекло iPhone: Liquid Glass на iOS 26, раньше — матовые поверхности", wrap: true) {
                    KToggle(isOn: Binding(get: { a.glass }, set: { v in set(a, glass: v) }))
                }
            }

            if a.theme == "custom" {
                KSectionHead("Основа")
                KSeg(value: Binding(get: { a.base }, set: { v in set(a, base: v) }), options: Palette.bases).padding(.top, 10)
                KSectionHead("Акцент")
                HStack(spacing: 0) {
                    ForEach(Palette.accents, id: \.self) { c in
                        swatch(c, on: a.accent.lowercased() == Palette.hex(c)) { set(a, accent: Palette.hex(c)) }
                        if c != Palette.accents.last { Spacer(minLength: 0) }
                    }
                }
                .padding(.horizontal, 16).padding(.vertical, 14)
                .kCard()
                .padding(.top, 12)
                Text("Акцентом подсвечиваются кнопка подключения, переключатели и статус «подключено».")
                    .font(.system(size: 12)).foregroundColor(K.dim2).padding(.top, 12)
            }
        }
    }

    /// `.swatch`: кружок 32, выбранный — в кольце своего цвета.
    private func swatch(_ c: UInt32, on: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Circle().fill(Color(hex: c)).frame(width: 32, height: 32)
                .padding(2).background(Circle().fill(K.card))
                .padding(2).background(Circle().fill(on ? Color(hex: c) : Color.clear))
        }
        .buttonStyle(Press())
        .accessibilityLabel("Акцент \(Palette.hex(c))")
    }

    private func set(_ a: Appearance, theme: String? = nil, base: String? = nil, accent: String? = nil, glass: Bool? = nil) {
        var n = a
        if let theme { n.theme = theme }
        if let base { n.base = base }
        if let accent { n.accent = accent }
        if let glass { n.glass = glass }
        engine.setAppearance(n)
    }
}

/// `.theme-card`: превью темы (полоса с точкой акцента и карточка) и название с кружком выбора.
private struct ThemeCard: View {
    let kind: String
    let label: String
    let a: Appearance
    let action: () -> Void

    private var palette: [String: UInt32] { Palette.palettes[kind == "light" ? "light" : kind == "custom" ? a.base : "graphite"] ?? [:] }
    private var accent: Color {
        kind == "custom" ? Color(hex: UInt32(a.accent.dropFirst(), radix: 16) ?? 0x30d158) : kind == "light" ? Color(hex: 0x1fa34a) : Color(hex: 0x30d158)
    }

    var body: some View {
        let on = a.theme == kind
        let sys = kind == "system"
        Button(action: action) {
            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 8) {
                    HStack(spacing: 8) {
                        Circle().fill(accent).frame(width: 16, height: 16)
                        RoundedRectangle(cornerRadius: 2).fill(sys ? Color.gray.opacity(0.6) : Color(hex: palette["dim"] ?? 0)).frame(width: 40, height: 4)
                    }
                    RoundedRectangle(cornerRadius: 8, style: .continuous).fill(sys ? Color.gray.opacity(0.28) : Color(hex: palette["card"] ?? 0))
                }
                .padding(8)
                .frame(height: 72)
                .background(preview(sys))
                .clipShape(RoundedRectangle(cornerRadius: K.r(12), style: .continuous))
                .overlay(RoundedRectangle(cornerRadius: K.r(12), style: .continuous)
                    .strokeBorder(sys ? Color.gray.opacity(0.3) : Color(hex: palette["line"] ?? 0), lineWidth: 1))
                HStack(spacing: 8) {
                    KRadio(on: on)
                    Text(label).font(.system(size: 14, weight: .medium)).foregroundColor(K.text)
                }
                .padding(.horizontal, 4).padding(.top, 12).padding(.bottom, 4)
            }
            .padding(8)
            .kCard()
            .overlay(RoundedRectangle(cornerRadius: K.r(20), style: .continuous).strokeBorder(on ? K.ring : Color.clear, lineWidth: 1))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private func preview(_ sys: Bool) -> some View {
        if sys {
            LinearGradient(stops: [.init(color: Color(hex: 0xf5f5f7), location: 0.5), .init(color: Color(hex: 0x1a1a1d), location: 0.5)],
                           startPoint: .topLeading, endPoint: .bottomTrailing)
        } else {
            Color(hex: palette["win"] ?? 0)
        }
    }
}

private struct LogScreen: View {
    @EnvironmentObject var engine: Engine
    @State private var lines: [LogLine]?
    let onBack: () -> Void

    var body: some View {
        KScreen {
            BackButton(label: "Настройки", action: onBack)
            KTitle("Журнал")
            if let lines {
                if lines.isEmpty {
                    Text("Записей пока нет").font(.system(size: 13)).foregroundColor(K.dim)
                        .frame(maxWidth: .infinity).padding(22).kCard(14).padding(.top, 16)
                } else {
                    VStack(alignment: .leading, spacing: 0) {
                        ForEach(Array(lines.reversed().enumerated()), id: \.offset) { i, l in
                            if i > 0 { KDivider() }
                            HStack(alignment: .top, spacing: 10) {
                                Text(time(l.at)).font(.system(size: 11, design: .monospaced)).foregroundColor(K.dim2)
                                Text(l.text).font(.system(size: 12)).foregroundColor(l.level == "error" ? K.red : l.level == "warn" ? K.orange : K.text2)
                                    .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: 0)
                            }
                            .padding(.horizontal, 14).padding(.vertical, 10)
                        }
                    }
                    .kCard()
                    .padding(.top, 16)
                }
            } else {
                ProgressView().tint(K.dim).frame(maxWidth: .infinity).padding(.top, 40)
            }
        }
        .task { lines = await engine.log() }
    }

    private func time(_ ms: Double) -> String {
        let f = DateFormatter()
        f.dateFormat = "HH:mm:ss"
        return f.string(from: Date(timeIntervalSince1970: ms / 1000))
    }
}

private struct AboutScreen: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.openURL) private var openURL
    @State private var about: About?
    let onBack: () -> Void

    private let authors = [
        ("vbu00", "Разработка", "https://github.com/vbu00"),
        ("Dmitriy Medvedev", "Дизайн интерфейса и логотип", "https://github.com/aleuuu"),
        ("limeflash", "Порты на macOS и iOS", "https://github.com/limeflash"),
    ]

    var body: some View {
        KScreen {
            BackButton(label: "Настройки", action: onBack)
            VStack(spacing: 0) {
                KIcon("power", size: 32).foregroundColor(K.accent)
                    .frame(width: 72, height: 72)
                    .kSurface(RoundedRectangle(cornerRadius: K.r(20), style: .continuous))
                Text("kl!ck").font(.system(size: 22, weight: .bold)).foregroundColor(K.text).padding(.top, 12)
                Text("Версия \(about?.version ?? "…") · бета с родным интерфейсом").font(.system(size: 12)).foregroundColor(K.dim).padding(.top, 4)
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 18)

            KCardList(top: 24) {
                kv("Ядро", "Mihomo \(about?.coreVersion ?? "встроено")")
                KDivider()
                kv("Система", "iOS \(UIDevice.current.systemVersion)")
            }

            KSectionHead("Авторы")
            KCardList {
                ForEach(Array(authors.enumerated()), id: \.offset) { i, a in
                    if i > 0 { KDivider() }
                    KRow(a.0, sub: a.1, chevron: true) { if let url = URL(string: a.2) { openURL(url) } }
                }
            }

            KCardList(top: 22) {
                KRow("Исходный код", sub: "github.com/vbu00/klick", chevron: true) {
                    if let url = URL(string: "https://github.com/vbu00/klick") { openURL(url) }
                }
            }
        }
        .task { about = try? await engine.get(About.self, "about") }
    }

    private func kv(_ k: String, _ v: String) -> some View {
        HStack {
            Text(k).foregroundColor(K.dim)
            Spacer()
            Text(v).foregroundColor(K.text).lineLimit(1)
        }
        .font(.system(size: 13))
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }
}
