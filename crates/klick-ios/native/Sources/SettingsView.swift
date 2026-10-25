import SwiftUI

/// Настройки: восстановление VPN, маршрутизация, подписки, оформление — карточками, как в веб-версии.
/// Остальное (Kill Switch, списки сайтов, журнал) — пока в обычном kl!ck.
struct SettingsView: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.openURL) private var openURL
    @AppStorage("theme") private var theme = "dark"
    @AppStorage("glass") private var glass = true
    @State private var about: About?

    private static let themes = [("dark", "Тёмная"), ("light", "Светлая"), ("system", "Как в системе")]

    var body: some View {
        KScreen(title: "Настройки") {
            if let s = engine.settings {
                KSection("VPN")
                VStack(spacing: 0) {
                    KRow(title: "Восстанавливать подключение", subtitle: "Если VPN был включён, после перезагрузки iPhone он включится сам") {
                        KToggle(isOn: Binding(
                            get: { s.restoreOnLogon ?? false },
                            set: { v in Task { await engine.setPref("restore_on_logon", v) } }
                        ))
                    }
                }
                .kCard()

                KSection("Маршрутизация")
                VStack(spacing: 0) {
                    KRow(title: "Через VPN", subtitle: s.routing == "all_vpn" ? "Весь трафик, кроме исключений" : "Только «Заблокированное в РФ» и сайты из списка") {
                        Menu {
                            Picker("Через VPN", selection: Binding(get: { s.routing }, set: { r in Task { await engine.setRouting(r) } })) {
                                Text("Весь трафик").tag("all_vpn")
                                Text("Только выбранное").tag("selected")
                            }
                        } label: {
                            KMenuValue(text: s.routing == "all_vpn" ? "Весь трафик" : "Выбранное")
                        }
                    }
                    if s.routing == "all_vpn" {
                        KDivider()
                        KRow(title: ".ru и .рф напрямую", subtitle: "Российские сайты — мимо VPN") {
                            KToggle(isOn: Binding(
                                get: { s.russiaDirect?.domains ?? false },
                                set: { v in Task { await engine.setPref("ru_domains", v) } }
                            ))
                        }
                        KDivider()
                        KRow(title: "Российские IP напрямую", subtitle: "Сервисы на адресах в России — мимо VPN") {
                            KToggle(isOn: Binding(
                                get: { s.russiaDirect?.ips ?? false },
                                set: { v in Task { await engine.setPref("ru_ips", v) } }
                            ))
                        }
                    }
                }
                .kCard()
                KFootnote(s.routing == "all_vpn"
                    ? "Заблокированное в РФ идёт через VPN всегда, даже с «российским напрямую»."
                    : "Список сайтов правится в обычном kl!ck.")

                if !s.connections.isEmpty {
                    KSection("Подписки")
                    VStack(spacing: 0) {
                        ForEach(Array(s.connections.enumerated()), id: \.element.id) { i, c in
                            if i > 0 { KDivider() }
                            KRow(title: c.name, subtitle: c.kind == "subscription" ? updated(c) : "одиночный сервер") {
                                Menu {
                                    if c.kind == "subscription" {
                                        Button { Task { await engine.refresh(c.id) } } label: { Label("Обновить", systemImage: "arrow.clockwise") }
                                    }
                                    Button(role: .destructive) { Task { await engine.remove(c.id) } } label: { Label("Удалить", systemImage: "trash") }
                                } label: {
                                    Image(systemName: "ellipsis").font(.system(size: 16, weight: .semibold)).foregroundColor(K.dim)
                                        .frame(width: 36, height: 36)
                                }
                            }
                        }
                    }
                    .kCard()
                }
            }

            KSection("Оформление")
            VStack(spacing: 0) {
                KRow(title: "Тема", subtitle: nil) {
                    Menu {
                        Picker("Тема", selection: $theme) {
                            ForEach(Self.themes, id: \.0) { Text($0.1).tag($0.0) }
                        }
                    } label: {
                        KMenuValue(text: Self.themes.first { $0.0 == theme }?.1 ?? "Тёмная")
                    }
                }
                KDivider()
                KRow(title: "Стекло", subtitle: "Системное стекло iPhone: Liquid Glass на iOS 26, раньше — матовые поверхности") {
                    KToggle(isOn: $glass.animation(.easeInOut(duration: 0.25)))
                }
            }
            .kCard()

            KSection("О kl!ck")
            VStack(spacing: 0) {
                KRow(title: "Версия", subtitle: nil) {
                    Text(Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "—")
                        .font(.system(size: 14)).foregroundColor(K.dim)
                }
                KDivider()
                KRow(title: "Ядро Mihomo", subtitle: about?.coreVersion ?? "встроено") {
                    Circle().fill(K.accent).frame(width: 7, height: 7)
                }
                KDivider()
                Button {
                    if let url = URL(string: "https://github.com/vbu00/klick") { openURL(url) }
                } label: {
                    KRow(title: "Исходный код", subtitle: "github.com/vbu00/klick") {
                        Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold)).foregroundColor(K.dim2)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
            .kCard()
            KFootnote("Бета kl!ck с родным интерфейсом iOS. Kill Switch, списки сайтов и журнал пока — в обычном kl!ck.")
        }
        .task { about = try? await engine.get(About.self, "about") }
    }

    private func updated(_ c: Connection) -> String {
        guard let t = c.updatedAt else { return "подписка" }
        let f = RelativeDateTimeFormatter()
        f.locale = Locale(identifier: "ru_RU")
        return "обновлена " + f.localizedString(for: Date(timeIntervalSince1970: t), relativeTo: Date())
    }
}
