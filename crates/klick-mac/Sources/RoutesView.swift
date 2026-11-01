import SwiftUI

/// «Маршруты»: что пускать через VPN — карточками, российское напрямую — одним тумблером, мой список,
/// «Сейчас» с программами и метками VPN / Напрямую / Частично, «Что-то не открывается?» → «Диагностика».
struct RoutesView: View {
    @EnvironmentObject var engine: Engine
    enum Sub { case list, live, diag }
    @State private var sub: Sub?
    @State private var conns: [LiveConn] = []
    @State private var fails: [Failure] = []

    var body: some View {
        Group {
            switch sub {
            case nil: main
            case .list: MyListScreen { sub = nil }
            case .live: LiveScreen(conns: conns) { sub = nil }
            case .diag: DiagnosticsScreen(back: "Маршруты", fails: fails) { sub = nil }
            }
        }
        // Соединения и неудачи раз в 2 секунды, пока открыт раздел и работает VPN.
        .task(id: engine.isRunning) {
            guard engine.isRunning else { conns = []; fails = []; return }
            while !Task.isCancelled {
                if let c = await engine.connections() { conns = c }
                if let f = await engine.failures() { fails = f }
                try? await Task.sleep(for: .seconds(2))
            }
        }
    }

    @ViewBuilder private var main: some View {
        if let s = engine.settings {
            let routing = s.routing
            let list = routing == "all_vpn" ? s.lists?.allVpn ?? [] : s.lists?.selected ?? []
            Screen {
                ScreenTitle(text: "Маршруты")
                Caption(text: "Что пускать через VPN")
                VStack(spacing: 8) {
                    Choice(on: routing == "all_vpn", title: "Всё, кроме исключений", sub: "Российские сайты и ваши исключения — напрямую") { Task { await engine.setRouting("all_vpn") } }
                    Choice(on: routing == "selected", title: "Только выбранное", sub: "Заблокированное в РФ и то, что добавите сами") { Task { await engine.setRouting("selected") } }
                }

                if routing == "all_vpn" {
                    Caption(text: "Напрямую, без VPN")
                    CardList {
                        Row(title: "Российские сайты и сервисы", sub: "Госуслуги, банки, маркетплейсы, VK — без VPN") {
                            let ru = s.russiaDirect.map { $0.domains || $0.ips } ?? false
                            Switch(on: ru) { v in Task { await engine.setRussia(v) } }
                        }
                        Row(title: "Мои исключения", sub: list.isEmpty ? "Сайты, сервисы и программы мимо VPN" : summary(list), tile: list.first.map { title($0) }, action: { sub = .list })
                    }
                } else {
                    Caption(text: "Через VPN")
                    CardList {
                        Row(title: "Заблокированное в РФ", sub: "Готовый набор, обновляется сам") {
                            Switch(on: s.blockedPreset != false) { v in Task { await engine.setPref("blocked_preset", v) } }
                        }
                        Row(title: "Мой список", sub: list.isEmpty ? "Добавьте сервис, программу или сайт" : summary(list), tile: list.first.map { title($0) }, action: { sub = .list })
                    }
                }
                Text("Локальная сеть — роутер, принтер, NAS — всегда идёт напрямую.").font(.system(size: 12.5)).foregroundStyle(K.text3).padding(.horizontal, 6).padding(.top, 8)

                Caption(text: "Сейчас", link: engine.isRunning ? "Все программы" : nil) { sub = .live }
                LiveSummary(conns: conns, running: engine.isRunning)

                CardList {
                    Row(title: "Что-то не открывается?", sub: "Диагностика: серверы, блокировки, утечки", action: { sub = .diag }) {
                        if !fails.isEmpty { Text("\(fails.count)").font(.system(size: 13, weight: .semibold)).foregroundStyle(K.orange) }
                    }
                }
                .padding(.top, 12)
            }
        }
    }

    private func title(_ r: Rule) -> String {
        r.target.kind == "service" ? engine.catalog.first { $0.id == r.target.value }?.name ?? r.target.value : r.target.value
    }

    private func summary(_ list: [Rule]) -> String {
        let n = { (kinds: [String]) in list.filter { kinds.contains($0.target.kind) }.count }
        return [(n(["service"]), ("сервис", "сервиса", "сервисов")), (n(["program"]), ("программа", "программы", "программ")), (n(["domain", "ip"]), ("сайт", "сайта", "сайтов"))]
            .filter { $0.0 > 0 }.map { Texts.plural($0.0, $0.1) }.joined(separator: ", ")
    }
}

/// Карточка выбора с кружком; выбранная — обводкой акцента.
struct Choice: View {
    var on: Bool
    var title: String
    var sub: String
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Radio(on: on)
                VStack(alignment: .leading, spacing: 2) {
                    Text(title).font(.system(size: 15, weight: .semibold)).foregroundStyle(K.text)
                    Text(sub).font(.system(size: 12.5)).foregroundStyle(K.text2).multilineTextAlignment(.leading)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            .background(RoundedRectangle(cornerRadius: 22, style: .continuous).fill(K.surface))
            .overlay(RoundedRectangle(cornerRadius: 22, style: .continuous).stroke(on ? K.ok : K.line, lineWidth: on ? 1.5 : 0.5))
        }
        .buttonStyle(PressStyle(scale: 0.985))
    }
}

/// Программа в «Сейчас»: соединения, байты, сколько через VPN.
struct AppRow: Identifiable {
    var name: String
    var conns: Int
    var vpn: Int
    var bytes: Double
    var hosts: [LiveConn]
    var id: String { name }

    static func group(_ list: [LiveConn]) -> [AppRow] {
        Dictionary(grouping: list) { $0.process ?? "Другие" }
            .map { AppRow(name: $0.key, conns: $0.value.count, vpn: $0.value.filter { $0.route == "vpn" }.count, bytes: $0.value.reduce(0) { $0 + $1.upload + $1.download }, hosts: $0.value) }
            .sorted { $0.bytes > $1.bytes }
    }
}

struct RouteTag: View {
    var vpn: Int
    var total: Int
    var body: some View {
        let (text, color): (String, Color) = vpn == total ? ("VPN", K.ok) : vpn == 0 ? ("Напрямую", K.text2) : ("Частично", K.orange)
        Text(text).font(.system(size: 11.5, weight: .medium)).foregroundStyle(color)
            .padding(.horizontal, 9).frame(height: 22)
            .background(Capsule().fill(color.opacity(vpn == 0 && vpn != total ? 0.12 : 0.14)))
    }
}

struct LiveSummary: View {
    var conns: [LiveConn]
    var running: Bool

    var body: some View {
        let apps = AppRow.group(conns)
        let vpn = conns.filter { $0.route == "vpn" }.count
        VStack(alignment: .leading, spacing: 0) {
            VStack(alignment: .leading, spacing: 8) {
                HStack(alignment: .firstTextBaseline, spacing: 6) {
                    Text(Texts.plural(conns.count, ("соединение", "соединения", "соединений"))).font(.system(size: 15, weight: .semibold)).foregroundStyle(K.text)
                    Text(Texts.plural(apps.count, ("программа", "программы", "программ"))).font(.system(size: 13)).foregroundStyle(K.text2)
                }
                GeometryReader { g in
                    HStack(spacing: 2) {
                        Capsule().fill(K.ok).frame(width: conns.isEmpty ? 0 : g.size.width * CGFloat(vpn) / CGFloat(conns.count))
                        Capsule().fill(K.fill)
                    }
                }
                .frame(height: 8)
                HStack(spacing: 14) {
                    Label("Через VPN \(vpn)", systemImage: "circle.fill").foregroundStyle(K.text2)
                    Label("Напрямую \(conns.count - vpn)", systemImage: "circle.fill").foregroundStyle(K.text2)
                }
                .font(.system(size: 12))
                .labelStyle(DotLabel())
                if !running { Text("Соединения появятся после подключения").font(.system(size: 12.5)).foregroundStyle(K.text3) }
            }
            .padding(16)
            ForEach(apps.prefix(3)) { a in
                Divider().overlay(K.line).padding(.leading, 16)
                HStack(spacing: 12) {
                    Tile(label: a.name)
                    VStack(alignment: .leading, spacing: 1) {
                        Text(a.name).font(.system(size: 14, weight: .medium)).foregroundStyle(K.text).lineLimit(1)
                        Text("\(a.conns) соед. · \(gb(a.bytes))").font(.system(size: 12)).foregroundStyle(K.text2)
                    }
                    Spacer()
                    RouteTag(vpn: a.vpn, total: a.conns)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
            }
        }
        .card(22)
    }
}

private struct DotLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 5) {
            configuration.icon.font(.system(size: 6))
            configuration.title
        }
    }
}

// MARK: Подэкраны

/// Мой список текущего положения: тумблер включает и выключает правило, крестик удаляет.
struct MyListScreen: View {
    @EnvironmentObject var engine: Engine
    var onBack: () -> Void
    @State private var adding = false

    var body: some View {
        let s = engine.settings
        let position = s?.routing ?? "selected"
        let list = position == "all_vpn" ? s?.lists?.allVpn ?? [] : s?.lists?.selected ?? []
        Screen {
            BackPill(label: "Маршруты", action: onBack)
            ScreenTitle(text: position == "all_vpn" ? "Мои исключения" : "Мой список").padding(.top, 8)
            Text(position == "all_vpn" ? "Это пойдёт мимо VPN." : "Это пойдёт через VPN, остальное — напрямую.").font(.system(size: 13)).foregroundStyle(K.text2).padding(.top, 4)
            if list.isEmpty {
                Text("Пока пусто").font(.system(size: 13)).foregroundStyle(K.text2).frame(maxWidth: .infinity).padding(22).card(18).padding(.top, 14)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(list.enumerated()), id: \.offset) { i, r in
                        if i > 0 { Divider().overlay(K.line).padding(.leading, 58) }
                        let name = r.target.kind == "service" ? engine.catalog.first { $0.id == r.target.value }?.name ?? r.target.value : r.target.value
                        HStack(spacing: 12) {
                            Tile(label: name).opacity(r.enabled == false ? 0.45 : 1)
                            Text(name).font(.system(size: 14, weight: .medium)).foregroundStyle(r.enabled == false ? K.text2 : K.text).lineLimit(1)
                            Spacer()
                            Switch(on: r.enabled != false) { v in Task { await engine.setRuleEnabled(position, i, v) } }
                            Button { Task { await engine.removeRule(position, i) } } label: {
                                Image(systemName: "xmark").font(.system(size: 11, weight: .semibold)).foregroundStyle(K.text3).frame(width: 24, height: 24)
                            }
                            .buttonStyle(.plain)
                            .help("Удалить")
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                    }
                }
                .card(22)
                .padding(.top, 14)
            }
            Button { adding = true } label: {
                Label("Добавить", systemImage: "plus").font(.system(size: 14, weight: .semibold)).foregroundStyle(K.text).frame(maxWidth: .infinity).frame(height: 44)
            }
            .buttonStyle(PressStyle(scale: 0.98))
            .card(14)
            .padding(.top, 12)
        }
        .sheet(isPresented: $adding) { AddRuleSheet(position: position).environmentObject(engine) }
    }
}

/// «Добавить в список»: сервисы каталога или сайт и IP.
struct AddRuleSheet: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.dismiss) private var dismiss
    var position: String
    @State private var picked: Set<String> = []
    @State private var site = ""

    var body: some View {
        let route = position == "selected" ? "vpn" : "direct"
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(position == "selected" ? "Через VPN" : "Мимо VPN").font(.system(size: 20, weight: .bold)).foregroundStyle(K.text)
                Spacer()
                Button("Отмена") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            ScrollView {
                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 8) {
                    ForEach(engine.catalog) { s in
                        let on = picked.contains(s.id)
                        Button { if on { picked.remove(s.id) } else { picked.insert(s.id) } } label: {
                            HStack(spacing: 8) {
                                Tile(label: s.name, size: 24)
                                Text(s.name).font(.system(size: 13, weight: .medium)).foregroundStyle(K.text).lineLimit(1)
                                Spacer(minLength: 0)
                                Image(systemName: on ? "checkmark.circle.fill" : "circle").foregroundStyle(on ? K.ok : K.text3)
                            }
                            .padding(10)
                            .background(RoundedRectangle(cornerRadius: 12, style: .continuous).fill(K.surface))
                            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).stroke(on ? K.ok : K.line, lineWidth: on ? 1 : 0.5))
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
            .frame(height: 220)
            TextField("Сайт или IP: example.com 1.2.3.4", text: $site).textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Добавить") {
                    let sites = site.split(whereSeparator: { " ,;\n\t".contains($0) }).map(String.init)
                    let rules = picked.map { Rule.json(.init(kind: "service", value: $0), route: route) }
                        + sites.map { v in Rule.json(.init(kind: v.range(of: #"^[\d.]+(/\d+)?$"#, options: .regularExpression) != nil || v.contains(":") ? "ip" : "domain", value: v), route: route) }
                    Task { if await engine.addRules(position, rules) > 0 { dismiss() } }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(picked.isEmpty && site.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(20)
        .frame(width: 380)
        .background(K.bg)
    }
}

/// «Сейчас в сети»: программы и их сайты; тумблер сайта пишет правило в мой список.
struct LiveScreen: View {
    @EnvironmentObject var engine: Engine
    var conns: [LiveConn]
    var onBack: () -> Void

    var body: some View {
        Screen {
            BackPill(label: "Маршруты", action: onBack)
            ScreenTitle(text: "Сейчас в сети").padding(.top, 8)
            Text("Тумблер включён — сайт идёт через VPN. Правило сохранится в мой список.").font(.system(size: 13)).foregroundStyle(K.text2).padding(.top, 4)
            ForEach(AppRow.group(conns)) { a in
                Caption(text: a.name)
                VStack(spacing: 0) {
                    ForEach(Array(a.hosts.enumerated()), id: \.offset) { i, h in
                        if i > 0 { Divider().overlay(K.line).padding(.leading, 16) }
                        HStack {
                            VStack(alignment: .leading, spacing: 1) {
                                Text(h.host).font(.system(size: 13.5, weight: .medium)).foregroundStyle(K.text).lineLimit(1)
                                Text(h.route == "vpn" ? "через VPN" : "напрямую").font(.system(size: 12)).foregroundStyle(h.route == "vpn" ? K.ok : K.text2)
                            }
                            Spacer()
                            Switch(on: h.route == "vpn") { v in
                                let ip = h.host.range(of: #"^[\d.]+$"#, options: .regularExpression) != nil
                                Task { await engine.routeTarget(.init(kind: ip ? "ip" : "domain", value: h.host), vpn: v, label: h.host) }
                            }
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                    }
                }
                .card(22)
            }
            if conns.isEmpty {
                Text(engine.isRunning ? "Ничего нет" : "VPN выключен. Соединения появятся после подключения.").font(.system(size: 13)).foregroundStyle(K.text2).frame(maxWidth: .infinity).padding(22).card(18).padding(.top, 14)
            }
        }
    }
}

/// «Диагностика»: недавние неудачи и «Как вас видят сайты».
struct DiagnosticsScreen: View {
    @EnvironmentObject var engine: Engine
    var back: String
    var fails: [Failure]
    var onBack: () -> Void
    @State private var report: IpReport?
    @State private var checking = false

    var body: some View {
        Screen {
            BackPill(label: back, action: onBack)
            ScreenTitle(text: "Диагностика").padding(.top, 8)
            Text("Что мешает открыть сайт: сервер, блокировка в этой сети или утечка мимо VPN.").font(.system(size: 13)).foregroundStyle(K.text2).padding(.top, 4)

            Caption(text: "Не открывается сейчас")
            if fails.isEmpty {
                Text("Неудачных соединений не было").font(.system(size: 13)).foregroundStyle(K.text2).frame(maxWidth: .infinity).padding(18).card(18)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(fails.enumerated()), id: \.offset) { i, f in
                        if i > 0 { Divider().overlay(K.line).padding(.leading, 16) }
                        HStack {
                            VStack(alignment: .leading, spacing: 1) {
                                Text(f.host).font(.system(size: 13.5, weight: .medium)).foregroundStyle(K.text).lineLimit(1)
                                Text("\(f.route == "vpn" ? "через VPN" : "напрямую") · \(f.error)").font(.system(size: 12)).foregroundStyle(K.text2)
                            }
                            Spacer()
                            Switch(on: f.route == "vpn") { v in Task { await engine.routeTarget(.init(kind: "domain", value: f.host), vpn: v, label: f.host) } }
                        }
                        .padding(.horizontal, 16)
                        .padding(.vertical, 10)
                    }
                }
                .card(22)
            }

            Caption(text: "Как вас видят сайты", link: report == nil ? nil : (checking ? "Проверяю…" : "Ещё раз")) { Task { await check() } }
            VStack(alignment: .leading, spacing: 12) {
                if let r = report {
                    HStack(alignment: .top, spacing: 10) {
                        column("Через VPN", r.viaVpn)
                        column("Напрямую", r.direct)
                    }
                    HStack(spacing: 14) {
                        flag(r.dnsProtected == true, "DNS защищён", "DNS мимо VPN")
                        flag(r.ipv6Leak != true, "IPv6 без утечки", "Утечка IPv6")
                    }
                } else {
                    Text("Какой адрес, страну и провайдера видят сайты — через VPN и напрямую. Заодно проверим, не уходят ли IPv6 и DNS мимо туннеля.").font(.system(size: 13)).foregroundStyle(K.text2)
                    Button(checking ? "Проверяю…" : "Проверить") { Task { await check() } }.disabled(checking)
                }
            }
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .card(22)
        }
    }

    private func check() async {
        checking = true
        report = await engine.ipCheck()
        checking = false
    }

    private func column(_ title: String, _ c: IpColumn?) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(title).caption()
            HStack(spacing: 6) {
                FlagBadge(code: c?.countryCode, size: 20)
                Text(c?.country ?? "—").font(.system(size: 14, weight: .semibold)).foregroundStyle(K.text)
            }
            Text(c?.ipv4 ?? c?.error ?? "—").font(.system(size: 12, design: .monospaced)).foregroundStyle(K.text2)
            if let p = c?.provider { Text(p).font(.system(size: 12)).foregroundStyle(K.text2).lineLimit(1) }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func flag(_ good: Bool, _ ok: String, _ bad: String) -> some View {
        Label(good ? ok : bad, systemImage: good ? "checkmark.shield" : "exclamationmark.triangle").font(.system(size: 12.5)).foregroundStyle(good ? K.ok : K.orange)
    }
}
