import SwiftUI
import UIKit

/// «Настройки» — как ui/src/screens/Settings.tsx на iOS: группы карточек и подэкраны «Если сервер
/// недоступен», «Тема», «Журнал», «О приложении».
struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @State private var sub: String?

    var body: some View {
        Group {
            switch sub {
            case "down": ServerDownScreen { sub = nil }
            case "theme": ThemeScreen { sub = nil }
            case "log": LogScreen { sub = nil }
            case "about": AboutScreen { sub = nil }
            default: SettingsMain { sub = $0 }
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

private let themeNames = [("system", "Системная"), ("light", "Светлая"), ("dark", "Тёмная")]

private struct SettingsMain: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.colorScheme) private var scheme
    @AppStorage("theme") private var theme = "dark"
    @State private var about: About?
    @State private var logCount: Int?
    @State private var lang = false
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
        let name = themeNames.first { $0.0 == theme }?.1 ?? "Тёмная"
        return theme == "system" ? "\(name) · сейчас \(scheme == .dark ? "тёмная" : "светлая")" : name
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
            .overlay(RoundedRectangle(cornerRadius: 20, style: .continuous).strokeBorder(on ? K.ring : Color.clear, lineWidth: 1))
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

private struct ThemeScreen: View {
    @AppStorage("theme") private var theme = "dark"
    @AppStorage("glass") private var glass = true
    let onBack: () -> Void

    private let desc = [
        "system": "Повторяет тему iOS и переключается вместе с ней.",
        "light": "Светлый фон и тёмный текст — удобно при ярком освещении.",
        "dark": "Графитовый фон — спокойно для глаз вечером и ночью.",
    ]

    var body: some View {
        KScreen {
            BackButton(label: "Настройки", action: onBack)
            KTitle("Тема")
            VStack(spacing: 8) {
                ForEach(themeNames, id: \.0) { k, name in
                    ChoiceCard(title: name, desc: desc[k], on: theme == k) { withAnimation { theme = k } }
                }
            }
            .padding(.top, 16)
            KCardList(top: 12) {
                KRow(title: "Стекло", sub: "Системное стекло iPhone: Liquid Glass на iOS 26, раньше — матовые поверхности", wrap: true) {
                    KToggle(isOn: $glass.animation(.easeInOut(duration: 0.25)))
                }
            }
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
                    .kSurface(RoundedRectangle(cornerRadius: 20, style: .continuous))
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
