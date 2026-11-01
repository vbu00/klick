import AppKit
import ServiceManagement
import SwiftUI

/// «Настройки»: подключение (способ, Kill Switch, если сервер недоступен), оформление, общие, диагностика.
struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    enum Sub { case log, diag, about }
    @State private var sub: Sub?
    @State private var about: About?
    @State private var loginItem = SMAppService.mainApp.status == .enabled

    var body: some View {
        switch sub {
        case .log: LogScreen { sub = nil }
        case .diag: DiagnosticsScreen(back: "Настройки", fails: []) { sub = nil }
        case .about: AboutScreen(about: about) { sub = nil }
        case nil: main
        }
    }

    @ViewBuilder private var main: some View {
        if let s = engine.settings {
            let a = s.appearance ?? .standard
            Screen {
                ScreenTitle(text: "Настройки")

                if let update = engine.state?.update {
                    CardList {
                        Row(title: "Доступна kl!ck \(update)", sub: "Обновите обычный kl!ck — служба и это окно возьмут новую версию")
                    }
                    .padding(.top, 14)
                }

                Caption(text: "Подключение")
                CardList {
                    Row(title: "Способ подключения", sub: engine.mode == "tun" ? "Весь трафик компьютера, включая игры и UDP" : "Браузеры и программы с системным прокси") {
                        Picker("", selection: Binding(get: { engine.mode }, set: { m in Task { await engine.setMode(m) } })) {
                            Text("VPN (TUN)").tag("tun")
                            Text("Прокси").tag("sys_proxy")
                        }
                        .labelsHidden().pickerStyle(.menu).fixedSize()
                    }
                    Row(title: "Kill Switch", sub: ksSub) {
                        Switch(on: engine.extras?.killSwitch?.enabled ?? false) { v in Task { await engine.setKillSwitch(v) } }
                    }
                    Row(title: "Если сервер недоступен", sub: downSub(s.onServerDown)) {
                        Picker("", selection: Binding(get: { s.onServerDown ?? "reconnect" }, set: { v in Task { await engine.setPref("on_server_down", v) } })) {
                            Text("Ждать").tag("reconnect")
                            Text("Следующий").tag("next_working")
                            Text("Самый быстрый").tag("fastest")
                        }
                        .labelsHidden().pickerStyle(.menu).fixedSize()
                    }
                    Row(title: "Восстанавливать подключение", sub: "Если VPN был включён, после перезагрузки он включится сам") {
                        Switch(on: s.restoreOnLogon ?? false) { v in Task { await engine.setPref("restore_on_logon", v) } }
                    }
                }

                Caption(text: "Оформление")
                CardList {
                    Row(title: "Тема") {
                        Picker("", selection: Binding(get: { a.theme == "custom" ? "system" : a.theme }, set: { v in var n = a; n.theme = v; engine.setAppearance(n) })) {
                            Text("Системная").tag("system")
                            Text("Светлая").tag("light")
                            Text("Тёмная").tag("dark")
                        }
                        .labelsHidden().pickerStyle(.segmented).fixedSize()
                    }
                    Row(title: "Акцент") {
                        HStack(spacing: 8) {
                            ForEach(["#34c759", "#0a84ff", "#bf5af2", "#ff9f0a", "#64d2ff", "#ff375f"], id: \.self) { hex in
                                let on = a.accent.lowercased() == hex
                                Button { var n = a; n.accent = hex; engine.setAppearance(n) } label: {
                                    Circle().fill(Color(nsColor: NSColor(srgbRed: CGFloat((K.parseHex(hex)! >> 16) & 0xff) / 255, green: CGFloat((K.parseHex(hex)! >> 8) & 0xff) / 255, blue: CGFloat(K.parseHex(hex)! & 0xff) / 255, alpha: 1)))
                                        .frame(width: 18, height: 18)
                                        .overlay(Circle().stroke(K.text.opacity(on ? 0.6 : 0), lineWidth: 2).padding(-3))
                                }
                                .buttonStyle(.plain)
                            }
                        }
                    }
                    Row(title: "Серверы по странам", sub: "Сначала страны, внутри — их серверы") {
                        Switch(on: s.serversByCountry ?? false) { v in Task { await engine.setPref("servers_by_country", v) } }
                    }
                }

                Caption(text: "Общие")
                CardList {
                    Row(title: "Открывать при входе в систему", sub: "Окно kl!ck β — вместе с macOS") {
                        Switch(on: loginItem) { v in
                            do {
                                if v { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
                                loginItem = v
                            } catch {
                                engine.toast = Toast(text: "Не удалось изменить автозапуск", bad: true)
                            }
                        }
                    }
                    Row(title: "Обновлять подписки", sub: "Как просит панель, иначе раз в 12 часов") {
                        Switch(on: s.autoUpdate ?? true) { v in Task { await engine.setPref("auto_update", v) } }
                    }
                }

                Caption(text: "Диагностика")
                CardList {
                    Row(title: "Проверка утечек", sub: "Как вас видят сайты — через VPN и напрямую", action: { sub = .diag })
                    Row(title: "Журнал", sub: "Записи службы", action: { sub = .log })
                    Row(title: "Скопировать отчёт", sub: "Версии, режим и журнал — без ссылок и адресов сайтов", action: {
                        Task {
                            if let text = await engine.report() {
                                NSPasteboard.general.clearContents()
                                NSPasteboard.general.setString(text, forType: .string)
                                engine.toast = Toast("Отчёт скопирован")
                            } else {
                                engine.toast = Toast(text: "Служба не отдала отчёт", bad: true)
                            }
                        }
                    })
                }

                CardList {
                    Row(title: "О приложении", sub: "Версия \(about?.version ?? "…") · окно β", action: { sub = .about })
                }
                .padding(.top, 22)
            }
            .task { about = await engine.about() }
        }
    }

    private var ksSub: String {
        guard let ks = engine.extras?.killSwitch else { return "Программы без VPN не выйдут в сеть" }
        let n = ks.programs.filter(\.enabled).count
        return ks.enabled ? "Вкл · \(Texts.plural(n, ("программа", "программы", "программ"))) · список — в обычном kl!ck" : "Выкл"
    }

    private func downSub(_ v: String?) -> String {
        switch v {
        case "next_working": "Перейти на следующий рабочий сервер"
        case "fastest": "Всегда держаться самого быстрого"
        default: "Ждать и переподключаться"
        }
    }
}

struct LogScreen: View {
    @EnvironmentObject var engine: Engine
    var onBack: () -> Void
    @State private var lines: [LogLine]?

    var body: some View {
        Screen {
            BackPill(label: "Настройки", action: onBack)
            ScreenTitle(text: "Журнал").padding(.top, 8)
            if let lines {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(lines.reversed()) { l in
                        HStack(alignment: .top, spacing: 10) {
                            Text(Date(timeIntervalSince1970: l.at / 1000), format: .dateTime.hour().minute().second()).font(.system(size: 11, design: .monospaced)).foregroundStyle(K.text3)
                            Text(l.text).font(.system(size: 12)).foregroundStyle(l.level == "error" ? K.red : l.level == "warn" ? K.orange : K.text)
                        }
                        .padding(.horizontal, 14)
                        .padding(.vertical, 8)
                        Divider().overlay(K.line)
                    }
                }
                .card(22)
                .padding(.top, 14)
            } else {
                ProgressView().controlSize(.small).frame(maxWidth: .infinity).padding(.top, 40)
            }
        }
        .task { lines = await engine.log() }
    }
}

struct AboutScreen: View {
    var about: About?
    var onBack: () -> Void

    var body: some View {
        Screen {
            BackPill(label: "Настройки", action: onBack)
            VStack(spacing: 6) {
                Image(systemName: "key.fill").font(.system(size: 30)).foregroundStyle(K.ok).frame(width: 72, height: 72).glass(RoundedRectangle(cornerRadius: 20, style: .continuous))
                Text("kl!ck β").font(.system(size: 22, weight: .bold)).foregroundStyle(K.text)
                Text("Окно на SwiftUI · служба \(about?.version ?? "…")").font(.system(size: 12)).foregroundStyle(K.text2)
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 18)
            CardList {
                Row(title: "Ядро", sub: "Mihomo \(about?.coreVersion ?? "…")")
                Row(title: "Система", sub: about?.os ?? ProcessInfo.processInfo.operatingSystemVersionString)
                Row(title: "Дизайн", sub: "Dmitriy Medvedev")
                Row(title: "Исходный код", sub: "github.com/vbu00/klick", action: { NSWorkspace.shared.open(URL(string: "https://github.com/vbu00/klick")!) })
            }
            .padding(.top, 22)
        }
    }
}
