import SwiftUI

/// Настройки: восстановление VPN, маршрутизация, подписки, тема. Остальное (Kill Switch, списки сайтов,
/// журнал) — пока в веб-версии kl!ck.
struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @AppStorage("theme") private var theme = "system"
    @State private var about: About?

    var body: some View {
        NavigationView {
            Form {
                if let s = engine.settings {
                    Section {
                        Toggle("Восстанавливать подключение", isOn: Binding(
                            get: { s.restoreOnLogon ?? false },
                            set: { v in Task { await engine.setPref("restore_on_logon", v) } }
                        ))
                    } header: {
                        Text("VPN")
                    } footer: {
                        Text("Если VPN был включён, после перезагрузки iPhone он включится сам.")
                    }

                    Section {
                        Picker("Через VPN", selection: Binding(get: { s.routing }, set: { r in Task { await engine.setRouting(r) } })) {
                            Text("Весь трафик").tag("all_vpn")
                            Text("Только выбранное").tag("selected")
                        }
                        if s.routing == "all_vpn" {
                            Toggle(".ru и .рф напрямую", isOn: Binding(
                                get: { s.russiaDirect?.domains ?? false },
                                set: { v in Task { await engine.setPref("ru_domains", v) } }
                            ))
                            Toggle("Российские IP напрямую", isOn: Binding(
                                get: { s.russiaDirect?.ips ?? false },
                                set: { v in Task { await engine.setPref("ru_ips", v) } }
                            ))
                        }
                    } header: {
                        Text("Маршрутизация")
                    } footer: {
                        Text(s.routing == "all_vpn"
                            ? "Заблокированное в РФ идёт через VPN всегда, даже с «российским напрямую»."
                            : "Через VPN — только «Заблокированное в РФ» и сайты из списка. Список правится в веб-версии kl!ck.")
                    }

                    if !s.connections.isEmpty {
                        Section {
                            ForEach(s.connections) { c in
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(c.name)
                                    Text(c.kind == "subscription" ? updated(c) : "одиночный сервер")
                                        .font(.caption).foregroundStyle(.secondary)
                                }
                                .swipeActions {
                                    Button(role: .destructive) {
                                        Task { await engine.remove(c.id) }
                                    } label: {
                                        Label("Удалить", systemImage: "trash")
                                    }
                                    if c.kind == "subscription" {
                                        Button {
                                            Task { await engine.refresh(c.id) }
                                        } label: {
                                            Label("Обновить", systemImage: "arrow.clockwise")
                                        }
                                        .tint(.blue)
                                    }
                                }
                            }
                        } header: {
                            Text("Подписки")
                        } footer: {
                            Text("Смахните влево, чтобы обновить или удалить.")
                        }
                    }
                }

                Section {
                    Picker("Тема", selection: $theme) {
                        Text("Системная").tag("system")
                        Text("Светлая").tag("light")
                        Text("Тёмная").tag("dark")
                    }
                } header: {
                    Text("Оформление")
                }

                Section {
                    row("Версия", Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "—")
                    row("Ядро", about?.coreVersion.map { "mihomo \($0)" } ?? "…")
                    if let url = URL(string: "https://github.com/vbu00/klick") {
                        Link("Исходный код на GitHub", destination: url)
                    }
                } header: {
                    Text("О kl!ck")
                } footer: {
                    Text("Это бета kl!ck с родным интерфейсом iOS. Kill Switch, списки сайтов и журнал пока — в обычном kl!ck.")
                }
            }
            .navigationTitle("Настройки")
            .task { about = try? await engine.get(About.self, "about") }
        }
        .navigationViewStyle(.stack)
    }

    private func row(_ title: String, _ value: String) -> some View {
        HStack {
            Text(title)
            Spacer()
            Text(value).foregroundStyle(.secondary)
        }
    }

    private func updated(_ c: Connection) -> String {
        guard let t = c.updatedAt else { return "подписка" }
        let f = RelativeDateTimeFormatter()
        f.locale = Locale(identifier: "ru_RU")
        return "обновлена " + f.localizedString(for: Date(timeIntervalSince1970: t), relativeTo: Date())
    }
}
