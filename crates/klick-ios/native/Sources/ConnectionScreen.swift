import SwiftUI

/// «Соединение» — как ui/src/screens/Connection.tsx на iOS: куда направлять с готовыми наборами, мой список,
/// «Сейчас в сети», «Не открывается?», живая схема и «Как вас видят сайты». Везде один тумблер — «через VPN»:
/// в списке он включает и выключает правило, в «Сейчас в сети» сам пишет правило в мой список.
struct ConnectionScreen: View {
    @EnvironmentObject var engine: Engine
    @State private var sub: String?
    @State private var adding = false
    @State private var conns: [LiveConn] = []
    @State private var fails: [Failure] = []

    var body: some View {
        Group {
            if let s = engine.settings {
                let routing = s.routing
                let list = routing == "all_vpn" ? s.lists?.allVpn ?? [] : s.lists?.selected ?? []
                let sites = Live.sites(conns, list: list, catalog: engine.catalog)
                switch sub {
                case "list": MyList(routing: routing, list: list, onBack: { sub = nil }, onAdd: { adding = true })
                case "live": LiveScreen(sites: sites, onBack: { sub = nil })
                case "failures": FailuresScreen(fails: fails, list: list, onBack: { sub = nil })
                default: main(s, routing: routing, list: list, sites: sites)
                }
            }
        }
        .sheet(isPresented: $adding) {
            AddRuleSheet(position: engine.settings?.routing ?? "selected") { adding = false }
                .environmentObject(engine)
        }
        .task { await engine.loadCatalog() }
        // Соединения и неудачи раз в 2 секунды, пока открыта вкладка и работает VPN.
        .task(id: engine.isRunning) {
            guard engine.isRunning else {
                conns = []
                fails = []
                return
            }
            while !Task.isCancelled {
                if let c = await engine.connections() { conns = c }
                if let f = await engine.failures() { fails = f }
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }
    }

    private func main(_ s: Settings, routing: String, list: [Rule], sites: [SiteRow]) -> some View {
        KScreen {
            KTitle("Соединение", xl: true)

            CxHead("Куда направлять")
            KSeg(value: Binding(get: { routing }, set: { r in Task { await engine.setRouting(r) } }),
                 options: [("all_vpn", "VPN для всего"), ("selected", "VPN для выбранного")], height: 38, size: 14, weight: .semibold)
            VStack(spacing: 0) {
                if routing == "selected" {
                    Preset(title: "Заблокированное в РФ — через VPN", desc: "Готовый набор, обновляется сам. Остальное — напрямую.",
                           on: Binding(get: { s.blockedPreset != false }, set: { v in Task { await engine.setPrefs(["blocked_preset": v]) } }))
                } else {
                    Preset(title: ".ru и .рф напрямую", desc: "Госуслуги, банки, Яндекс — без VPN",
                           on: Binding(get: { s.russiaDirect?.domains ?? false }, set: { v in Task { await engine.setPref("ru_domains", v) } }))
                    CxLine()
                    Preset(title: "Российские IP напрямую", desc: "По базе стран — кроме заблокированного в РФ: оно всё равно через VPN",
                           on: Binding(get: { s.russiaDirect?.ips ?? false }, set: { v in Task { await engine.setPref("ru_ips", v) } }))
                    CxLine()
                    Preset(title: "Локальная сеть — напрямую", desc: "Роутер, принтер, игры по локалке", on: nil)
                }
            }
            .kCard(14)
            .padding(.top, 8)

            CxHead("Мой список", wide: true)
            Button { sub = "list" } label: { listCard(routing, list) }.buttonStyle(.plain)
            Button { adding = true } label: {
                HStack(spacing: 8) {
                    KIcon("plus", size: 16)
                    Text("Добавить")
                }
            }
            .buttonStyle(CardButtonStyle(weight: .semibold))
            .padding(.top, 8)

            CxHead("Сейчас в сети", wide: true)
            Button { sub = "live" } label: { LiveSummary(sites: sites, running: engine.isRunning) }.buttonStyle(.plain)
            if !fails.isEmpty {
                Button { sub = "failures" } label: {
                    HStack(spacing: 12) {
                        VStack(alignment: .leading, spacing: 3) {
                            Text("Не открывается?").font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                            Text("Недавние неудачные соединения").font(.system(size: 12.5)).foregroundColor(K.dim)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        Text("\(fails.count)").font(.system(size: 13)).monospacedDigit().foregroundColor(K.orange)
                        KIcon("chevron", size: 14).foregroundColor(K.dim)
                    }
                    .padding(.horizontal, 16).padding(.vertical, 13)
                    .kCard(14)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .padding(.top, 8)
            }

            CxHead("Схема · сейчас", wide: true)
            LiveSchema(running: engine.isRunning, server: engine.state?.server, conns: conns, catalog: engine.catalog)

            IpCard()
        }
    }

    private func listCard(_ routing: String, _ list: [Rule]) -> some View {
        HStack(spacing: 14) {
            VStack(alignment: .leading, spacing: 0) {
                Text("\(routing == "selected" ? "Через VPN" : "Напрямую") · мой список")
                    .font(.system(size: 16, weight: .semibold)).foregroundColor(K.text)
                Text(list.isEmpty ? "Пока пусто — добавьте сервис или сайт" : Rules.summary(list))
                    .font(.system(size: 12.5)).foregroundColor(K.dim).padding(.top, 3)
                if !list.isEmpty {
                    HStack(spacing: -6) {
                        ForEach(Array(list.prefix(6).enumerated()), id: \.offset) { _, r in
                            RuleTile(label: Rules.title(r, engine.catalog), size: 28)
                                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(K.card, lineWidth: 2))
                                .opacity(r.enabled == false ? 0.4 : 1)
                        }
                        Text("включено \(list.filter { $0.enabled != false }.count) из \(list.count)")
                            .font(.system(size: 12)).foregroundColor(K.dim).padding(.leading, 20)
                    }
                    .padding(.top, 12)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            KIcon("chevron", size: 14).foregroundColor(K.dim)
        }
        .padding(16)
        .kCard(14)
        .contentShape(Rectangle())
    }
}

// MARK: Общее

/// `.cx-head`: подпись раздела «Соединения» — 12, полужирный, заглавными.
struct CxHead: View {
    let text: String
    var wide = false
    var small = false

    init(_ text: String, wide: Bool = false, small: Bool = false) {
        self.text = text
        self.wide = wide
        self.small = small
    }

    var body: some View {
        Text(text.uppercased()).font(.system(size: 12, weight: .semibold)).tracking(0.72).foregroundColor(K.dim)
            .padding(.horizontal, 4).padding(.top, small ? 18 : wide ? 22 : 20).padding(.bottom, 8)
    }
}

/// Линия между строками `.cx-card`.
private struct CxLine: View {
    var body: some View { Rectangle().fill(K.line).frame(height: 1) }
}

/// Готовый набор: название, пояснение и тумблер (или «всегда»).
private struct Preset: View {
    let title: String
    let desc: String
    let on: Binding<Bool>?

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                Text(desc).font(.system(size: 12)).foregroundColor(K.dim).lineSpacing(1).fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if let on { KToggle(isOn: on) } else { Text("всегда").font(.system(size: 12)).foregroundColor(K.dim2) }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }
}

/// Подзаголовок подэкрана (`.sub-title`, `.sub-lead`).
private struct SubHead: View {
    let title: String
    var lead: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title).font(.system(size: 24, weight: .semibold)).foregroundColor(K.text)
            if let lead { Text(lead).font(.system(size: 13)).foregroundColor(K.dim).lineSpacing(2).fixedSize(horizontal: false, vertical: true) }
        }
        .padding(.top, 12)
    }
}

/// `.cx-empty`.
private struct CxEmpty: View {
    let text: String
    var body: some View {
        Text(text).font(.system(size: 13)).foregroundColor(K.dim).multilineTextAlignment(.center).lineSpacing(2)
            .frame(maxWidth: .infinity).padding(22).kCard(14).padding(.top, 14)
    }
}

/// `.foot-note`.
private struct FootNote: View {
    let text: String
    var body: some View {
        Text(text).font(.system(size: 12)).foregroundColor(K.dim2).lineSpacing(2).fixedSize(horizontal: false, vertical: true).padding(.top, 12)
    }
}

/// Строка правила или сайта: плитка, название, подпись и тумблер.
private struct RuleRow<Trailing: View>: View {
    let label: String
    let title: String
    let sub: String
    var subVpn = false
    var dimmed = false
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 12) {
            RuleTile(label: label).opacity(dimmed ? 0.45 : 1)
            VStack(alignment: .leading, spacing: 0) {
                Text(title).font(.system(size: 14, weight: .semibold)).foregroundColor(dimmed ? K.dim : K.text).lineLimit(1)
                Text(sub).font(.system(size: 11.5)).foregroundColor(subVpn ? K.accent : K.dim).lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing
        }
        .padding(.leading, 14).padding(.trailing, 10).padding(.vertical, 11)
    }
}

// MARK: Правила

/// Названия, цвета и сводки правил — как ui/src/lib/rules.ts.
enum Rules {
    private static let tints: [UInt32] = [0xa8c7fa, 0xf5c26b, 0xc4b5fd, 0x9ad9c0, 0xf4a7b9, 0xb8c4d0]

    static func tint(_ key: String) -> Color {
        var h: UInt32 = 0
        for ch in key.utf16 { h = h &* 31 &+ UInt32(ch) }
        return Color(hex: tints[Int(h % UInt32(tints.count))])
    }

    static func title(_ r: Rule, _ catalog: [Service]) -> String { title(r.target, catalog) }

    static func title(_ t: Rule.Target, _ catalog: [Service]) -> String {
        switch t.kind {
        case "service": return catalog.first { $0.id == t.value }?.name ?? t.value
        case "domain": return t.value.contains(".") ? t.value : ".\(t.value)"
        case "ip": return t.value.replacingOccurrences(of: "/32", with: "").replacingOccurrences(of: "/128", with: "")
        default: return t.value
        }
    }

    /// «3 сервиса · 2 сайта».
    static func summary(_ list: [Rule]) -> String {
        let services = list.filter { $0.target.kind == "service" }.count
        let sites = list.filter { ["domain", "ip"].contains($0.target.kind) }.count
        return [services > 0 ? Texts.plural(services, ("сервис", "сервиса", "сервисов")) : nil,
                sites > 0 ? Texts.plural(sites, ("сайт", "сайта", "сайтов")) : nil].compactMap { $0 }.joined(separator: " · ")
    }

    static func routeName(_ r: String) -> String { r == "vpn" ? "через VPN" : r == "block" ? "блок" : "напрямую" }
}

/// `.tile-letter`: первая буква на цветной плитке.
struct RuleTile: View {
    let label: String
    var size: CGFloat = 30
    var body: some View {
        Text(label.trimmingCharacters(in: .whitespaces).prefix(1).uppercased())
            .font(.system(size: size < 30 ? 11 : 12, weight: .bold)).foregroundColor(Color(hex: 0x1a1a1d))
            .frame(width: size, height: size)
            .background(Rules.tint(label), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

// MARK: Мой список

private struct MyList: View {
    @EnvironmentObject var engine: Engine
    let routing: String
    let list: [Rule]
    let onBack: () -> Void
    let onAdd: () -> Void
    @State private var tab = "all"

    var body: some View {
        let enabled = list.filter { $0.enabled != false }.count
        let indexed = Array(list.enumerated())
        let groups: [(String, String, [(offset: Int, element: Rule)])] = [("service", "Сервисы"), ("site", "Сайты и IP")]
            .filter { tab == "all" || tab == $0.0 }
            .map { k, label in (k, label, indexed.filter { kind($0.element) == k }) }
            .filter { !$0.2.isEmpty }
        KScreen {
            BackButton(label: "Соединение", action: onBack)
            SubHead(title: "\(routing == "selected" ? "Через VPN" : "Напрямую") · мой список",
                    lead: "Положение «\(routing == "all_vpn" ? "VPN для всего" : "VPN для выбранного")». Включено \(enabled) из \(list.count). Выключенное правило не работает, но остаётся в списке.")
            KSeg(value: $tab, options: [("all", "Все"), ("service", "Сервисы"), ("site", "Сайты")], height: 32, size: 12.5, weight: .semibold)
                .padding(.top, 14)
            ForEach(groups, id: \.0) { _, label, rows in
                CxHead(label, small: true)
                VStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element.offset) { i, row in
                        if i > 0 { CxLine() }
                        rule(row.offset, row.element)
                    }
                }
                .kCard(14)
            }
            if groups.isEmpty {
                CxEmpty(text: routing == "selected" ? "Добавьте сервис или сайт — они пойдут через VPN." : "Добавьте то, что должно идти напрямую: банк, локальный сервис.")
            }
            Button(action: onAdd) {
                HStack(spacing: 8) {
                    KIcon("plus", size: 16)
                    Text("Добавить")
                }
            }
            .buttonStyle(CardButtonStyle(weight: .semibold))
            .padding(.top, 12)
        }
    }

    private func kind(_ r: Rule) -> String { ["domain", "ip"].contains(r.target.kind) ? "site" : r.target.kind }

    private func rule(_ index: Int, _ r: Rule) -> some View {
        let title = Rules.title(r, engine.catalog)
        let on = r.enabled != false
        let exception = (r.route ?? "") != (routing == "selected" ? "vpn" : "direct")
        return HStack(spacing: 12) {
            RuleTile(label: title).opacity(on ? 1 : 0.45)
            Text(title).font(.system(size: 14, weight: .semibold)).foregroundColor(on ? K.text : K.dim).lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
            if exception, let route = r.route {
                Text(Rules.routeName(route)).font(.system(size: 11, weight: .semibold))
                    .foregroundColor(route == "vpn" ? K.accent : route == "block" ? K.red : K.text2)
                    .padding(.horizontal, 8).frame(height: 22)
                    .background(route == "vpn" ? K.accent.opacity(0.16) : route == "block" ? K.red.opacity(0.16) : K.elem, in: Capsule())
            }
            KToggle(isOn: Binding(get: { on }, set: { v in Task { await engine.setRuleEnabled(routing, index, v) } }))
            Button {
                Task { await engine.removeRule(routing, index) }
            } label: {
                KIcon("win-close", size: 11).foregroundColor(K.dim2).frame(width: 26, height: 26).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Удалить правило")
        }
        .padding(.leading, 14).padding(.trailing, 10).padding(.vertical, 11)
    }
}

// MARK: Добавить в список

/// Лист «Добавить в список»: сервисы каталога или сайт и IP. Правила встают в список текущего положения:
/// в «VPN для выбранного» — через VPN, в «VPN для всего» — напрямую.
private struct AddRuleSheet: View {
    @EnvironmentObject var engine: Engine
    let position: String
    let onClose: () -> Void
    @State private var tab = "services"
    @State private var picked: [String] = []
    @State private var site = ""
    @State private var busy = false

    private var route: String { position == "selected" ? "vpn" : "direct" }
    private var list: [Rule] { position == "all_vpn" ? engine.settings?.lists?.allVpn ?? [] : engine.settings?.lists?.selected ?? [] }
    private var free: [Service] { engine.catalog.filter { s in !list.contains { $0.target.kind == "service" && $0.target.value.lowercased() == s.id.lowercased() } } }
    private var siteItems: [String] { site.split(whereSeparator: { " ,;\n\t".contains($0) }).map(String.init) }
    private var count: Int { picked.count + siteItems.count }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Добавить в «\(position == "selected" ? "Через VPN" : "Напрямую")»").font(.system(size: 20, weight: .bold)).foregroundColor(K.text)
                    Text(position == "selected" ? "Выбранное пойдёт через VPN, остальное — напрямую." : "Выбранное пойдёт мимо VPN: банк, игра, локальный сервис.")
                        .font(.system(size: 12.5)).foregroundColor(K.dim)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Button(action: onClose) {
                    KIcon("win-close", size: 12).foregroundColor(K.dim).frame(width: 32, height: 32).kSurface(Circle(), interactive: true)
                }
                .buttonStyle(.plain)
                .accessibilityLabel("Закрыть")
            }
            .padding(.horizontal, 18).padding(.top, 18)

            KSeg(value: $tab, options: [("services", "Сервисы"), ("site", "Сайт или IP")], height: 34, size: 13, weight: .semibold)
                .padding(.horizontal, 18).padding(.top, 14)

            ScrollView {
                if tab == "services" {
                    LazyVGrid(columns: [GridItem(.flexible(), spacing: 8), GridItem(.flexible(), spacing: 8)], spacing: 8) {
                        ForEach(free) { s in
                            let on = picked.contains(s.id)
                            Button {
                                if on { picked.removeAll { $0 == s.id } } else { picked.append(s.id) }
                            } label: {
                                HStack(spacing: 10) {
                                    RuleTile(label: s.name, size: 26)
                                    Text(s.name).font(.system(size: 13, weight: .semibold)).foregroundColor(K.text).lineLimit(1)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                    Check(on: on)
                                }
                                .padding(10)
                                .kCard(12)
                                .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(on ? K.accent : Color.clear, lineWidth: 1))
                                .contentShape(Rectangle())
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    if free.isEmpty {
                        Text(engine.catalog.isEmpty ? "Загружаю каталог…" : "Все сервисы каталога уже в списке")
                            .font(.system(size: 13)).foregroundColor(K.dim).padding(18)
                    }
                } else {
                    VStack(alignment: .leading, spacing: 8) {
                        TextField("", text: $site, prompt: Text(verbatim: "example.com или 1.2.3.4").foregroundColor(K.dim2))
                            .font(.system(size: 14, design: .monospaced))
                            .foregroundColor(K.text)
                            .textInputAutocapitalization(.never)
                            .disableAutocorrection(true)
                            .keyboardType(.URL)
                            .submitLabel(.done)
                            .onSubmit { Task { await submit() } }
                            .padding(.horizontal, 14).frame(height: 44)
                            .kCard(12)
                            .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(K.line, lineWidth: 1))
                        Text("Поддомены включаются автоматически.").font(.system(size: 12)).foregroundColor(K.dim).padding(.horizontal, 4)
                    }
                }
            }
            .padding(.horizontal, 18).padding(.vertical, 12)

            Button {
                Task { await submit() }
            } label: {
                Text(busy ? "Добавляю…" : count > 0 ? "Добавить · \(count)" : "Добавить")
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundColor(count > 0 ? K.onAccent : K.dim2)
                    .frame(maxWidth: .infinity).frame(height: 46)
                    .background(count > 0 ? K.accent : K.card, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            }
            .buttonStyle(Press())
            .disabled(count == 0 || busy)
            .padding(.horizontal, 18).padding(.bottom, 18)
        }
        .background(KBackground())
        .task { await engine.loadCatalog() }
    }

    private func submit() async {
        guard count > 0, !busy else { return }
        busy = true
        let rules = picked.map { Rule.json(Rule.Target(kind: "service", value: $0), route: route) }
            + siteItems.map { v in
                let ip = v.range(of: "^[\\d.]+(/\\d+)?$", options: .regularExpression) != nil || v.contains(":")
                return Rule.json(Rule.Target(kind: ip ? "ip" : "domain", value: v), route: route)
            }
        let added = await engine.addRules(position, rules)
        busy = false
        if added > 0 {
            engine.toast = Toast(added == 1 ? "Добавлено в список" : "Добавлено правил: \(added)", "Пойдёт \(Rules.routeName(route))")
            onClose()
        }
    }
}

/// `.ax-check`: квадратик выбора.
private struct Check: View {
    let on: Bool
    var body: some View {
        RoundedRectangle(cornerRadius: 5, style: .continuous)
            .fill(on ? K.accent : Color.clear)
            .overlay(RoundedRectangle(cornerRadius: 5, style: .continuous).strokeBorder(on ? K.accent : K.ring, lineWidth: 1.5))
            .overlay(
                Path { p in
                    p.move(to: CGPoint(x: 4.5, y: 9.3))
                    p.addLine(to: CGPoint(x: 7.2, y: 12))
                    p.addLine(to: CGPoint(x: 12.7, y: 6.2))
                }
                .stroke(K.onAccent, style: StrokeStyle(lineWidth: 2.2, lineCap: .round, lineJoin: .round))
                .opacity(on ? 1 : 0)
            )
            .frame(width: 18, height: 18)
    }
}

// MARK: Сейчас в сети

/// Сайт в «Сейчас в сети»: хост, сколько соединений и байт, куда идёт и какое правило запишет тумблер.
struct SiteRow: Identifiable {
    var host: String
    var conns: Int
    var bytes: Double
    var vpn: Bool
    var target: Rule.Target
    var id: String { host }
}

/// Разбор соединений — как ui/src/lib/live.ts, только сайты: iOS не говорит, какое приложение открыло соединение.
enum Live {
    static func isIp(_ h: String) -> Bool { h.range(of: "^[\\d.]+$", options: .regularExpression) != nil || h.contains(":") }

    private static func hostMatches(_ host: String, _ domain: String) -> Bool {
        let h = host.lowercased()
        let d = domain.lowercased().hasPrefix(".") ? String(domain.lowercased().dropFirst()) : domain.lowercased()
        return h == d || h.hasSuffix(".\(d)")
    }

    private static func ipInCidr(_ ip: String, _ cidr: String) -> Bool {
        let parts = cidr.split(separator: "/", maxSplits: 1).map(String.init)
        let net = parts.first ?? cidr
        func num(_ s: String) -> UInt32? {
            let o = s.split(separator: ".").compactMap { UInt32($0) }
            return o.count == 4 ? o.reduce(0) { ($0 << 8) | ($1 & 255) } : nil
        }
        guard let a = num(ip), let b = num(net) else { return ip.lowercased() == net.lowercased() }
        let bits = parts.count > 1 ? Int(parts[1]) ?? 32 : 32
        let mask: UInt32 = bits == 0 ? 0 : UInt32.max << UInt32(32 - bits)
        return a & mask == b & mask
    }

    static func service(for host: String, _ catalog: [Service]) -> Service? {
        let h = host.lowercased()
        return catalog.first { s in s.domains.contains { h == $0 || h.hasSuffix(".\($0)") } }
    }

    /// Сайт без поддоменов: две последние части, три — у `co.uk`, `com.ru` и похожих.
    static func baseDomain(_ host: String) -> String {
        var labels = host.lowercased().split(separator: ".").map(String.init)
        if host.hasSuffix(".") { labels = labels.filter { !$0.isEmpty } }
        guard labels.count > 2 else { return labels.joined(separator: ".") }
        let sld = labels[labels.count - 2], tld = labels[labels.count - 1]
        let three = tld.count == 2 && ["co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go", "msk", "spb"].contains(sld)
        return labels.suffix(three ? 3 : 2).joined(separator: ".")
    }

    /// Сайт для правила: `rr3.googlevideo.com` → сервис YouTube; `cdn.example.co.uk` → `example.co.uk`.
    static func target(_ host: String, _ catalog: [Service]) -> Rule.Target {
        if let s = service(for: host, catalog) { return Rule.Target(kind: "service", value: s.id) }
        if isIp(host) { return Rule.Target(kind: "ip", value: host) }
        return Rule.Target(kind: "domain", value: baseDomain(host))
    }

    /// Включённое правило сайта, сервиса или IP для хоста: точное раньше общего, своё раньше сервиса.
    static func rule(_ list: [Rule], _ host: String, _ catalog: [Service]) -> Rule? {
        var best: (Rule, Int)?
        for r in list where r.enabled != false {
            let t = r.target
            var rank = -1
            if t.kind == "domain" && hostMatches(host, t.value) {
                rank = t.value.split(separator: ".").count * 2 + 1
            } else if t.kind == "ip" && isIp(host) && ipInCidr(host, t.value) {
                rank = 100 + (Int(t.value.split(separator: "/").dropFirst().first ?? "32") ?? 32) * 2 + 1
            } else if t.kind == "service", let svc = catalog.first(where: { $0.id == t.value }) {
                if let d = svc.domains.filter({ hostMatches(host, $0) }).max(by: { $0.split(separator: ".").count < $1.split(separator: ".").count }) {
                    rank = d.split(separator: ".").count * 2
                } else if isIp(host) && svc.cidrs.contains(where: { ipInCidr(host, $0) }) {
                    rank = 100
                }
            }
            if rank >= 0 && (best == nil || rank > best!.1) { best = (r, rank) }
        }
        return best?.0
    }

    /// Сайты сейчас: сначала те, что качают больше. Куда идёт — по правилу из списка, иначе как решило ядро.
    static func sites(_ conns: [LiveConn], list: [Rule], catalog: [Service]) -> [SiteRow] {
        var map: [String: SiteRow] = [:]
        for c in conns where c.route != "block" {
            let raw = c.host.hasPrefix("www.") ? String(c.host.dropFirst(4)) : c.host
            let host = raw.isEmpty ? "—" : raw
            let bytes = c.download + c.upload
            if var s = map[host] {
                s.conns += 1
                s.bytes += bytes
                map[host] = s
            } else {
                let vpn = rule(list, c.host, catalog).map { $0.route == "vpn" } ?? (c.route == "vpn")
                map[host] = SiteRow(host: host, conns: 1, bytes: bytes, vpn: vpn, target: target(c.host, catalog))
            }
        }
        return map.values.sorted { $0.bytes > $1.bytes }
    }
}

/// Сводка «Сейчас в сети»: сколько соединений, доля VPN и самые активные сайты.
private struct LiveSummary: View {
    let sites: [SiteRow]
    let running: Bool

    var body: some View {
        let total = sites.reduce(0) { $0 + $1.conns }
        let vpn = sites.filter(\.vpn).reduce(0) { $0 + $1.conns }
        VStack(spacing: 0) {
            VStack(alignment: .leading, spacing: 0) {
                Text(Texts.plural(total, ("соединение", "соединения", "соединений"))).font(.system(size: 15, weight: .semibold)).foregroundColor(K.text)
                GeometryReader { g in
                    HStack(spacing: 2) {
                        if vpn > 0 { Rectangle().fill(K.accent).frame(width: total > 0 ? g.size.width * CGFloat(vpn) / CGFloat(total) : 0) }
                        if total > vpn { Rectangle().fill(K.text2) }
                    }
                }
                .frame(height: 8)
                .background(K.elem)
                .clipShape(RoundedRectangle(cornerRadius: 4))
                .padding(.top, 10)
                HStack(spacing: 14) {
                    legend(K.accent, "VPN", vpn)
                    legend(K.text2, "Напрямую", total - vpn)
                }
                .padding(.top, 8)
                if !running {
                    Text("Соединения появятся после подключения").font(.system(size: 12)).foregroundColor(K.dim2).padding(.top, 8)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 14).padding(.top, 14).padding(.bottom, 12)
            ForEach(sites.prefix(3)) { s in
                HStack(spacing: 12) {
                    RuleTile(label: s.host, size: 28)
                    VStack(alignment: .leading, spacing: 0) {
                        Text(s.host).font(.system(size: 13.5, weight: .semibold)).foregroundColor(K.text).lineLimit(1)
                        Text("\(s.conns) соед. · \(String(format: "%.1f", s.bytes / 1e6).replacingOccurrences(of: ".", with: ",")) МБ")
                            .font(.system(size: 11.5)).foregroundColor(K.dim)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Capsule().fill(s.vpn ? K.accent : K.text2).frame(width: 52, height: 5)
                }
                .padding(.horizontal, 14).padding(.vertical, 9)
                .overlay(alignment: .top) { CxLine() }
            }
        }
        .kCard(14)
        .contentShape(Rectangle())
    }

    private func legend(_ c: Color, _ title: String, _ n: Int) -> some View {
        HStack(spacing: 6) {
            Circle().fill(c).frame(width: 7, height: 7)
            (Text(title + " ") + Text("\(n)").fontWeight(.semibold).foregroundColor(K.text)).font(.system(size: 12)).foregroundColor(K.dim)
        }
    }
}

private struct LiveScreen: View {
    @EnvironmentObject var engine: Engine
    let sites: [SiteRow]
    let onBack: () -> Void
    @State private var filter = "all"
    @State private var paused = false
    @State private var frozen: [SiteRow] = []

    var body: some View {
        let shown = paused ? frozen : sites
        let nVpn = shown.filter(\.vpn).count
        let rows = shown.filter { filter == "all" || (filter == "vpn") == $0.vpn }
        KScreen {
            BackButton(label: "Соединение", action: onBack)
            HStack(spacing: 10) {
                Text("Сейчас в сети").font(.system(size: 24, weight: .semibold)).foregroundColor(K.text)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    if !paused { frozen = sites }
                    paused.toggle()
                } label: {
                    HStack(spacing: 7) {
                        Circle().frame(width: 7, height: 7)
                        Text(paused ? "Пауза" : "Live")
                    }
                    .font(.system(size: 12.5, weight: .semibold))
                    .foregroundColor(paused ? K.dim : K.accent)
                    .padding(.horizontal, 12).frame(height: 30)
                    .background(paused ? K.card : K.accent.opacity(0.14), in: Capsule())
                }
                .buttonStyle(Press())
                .disabled(!engine.isRunning)
            }
            .padding(.top, 14)
            Text("Тумблер включён — идёт через VPN. Правило сохранится в мой список.")
                .font(.system(size: 13)).foregroundColor(K.dim).padding(.top, 6)
            HStack(spacing: 6) {
                chip("all", "Все", shown.count, K.dim)
                chip("vpn", "VPN", nVpn, K.accent)
                chip("direct", "Прямо", shown.count - nVpn, K.text2)
            }
            .padding(.top, 10)
            if !engine.isRunning {
                CxEmpty(text: "VPN выключен. Соединения появятся после подключения.")
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(rows.enumerated()), id: \.element.id) { i, s in
                        if i > 0 { CxLine() }
                        RuleRow(label: s.host, title: s.host, sub: s.vpn ? "через VPN" : "напрямую", subVpn: s.vpn) {
                            KToggle(isOn: Binding(get: { s.vpn }, set: { v in
                                Task { await engine.routeTarget(s.target, vpn: v, label: Rules.title(s.target, engine.catalog)) }
                            }))
                        }
                    }
                    if rows.isEmpty {
                        Text("Ничего нет").font(.system(size: 13)).foregroundColor(K.dim).frame(maxWidth: .infinity).padding(22)
                    }
                }
                .kCard(14)
                .padding(.top, 10)
            }
        }
    }

    private func chip(_ key: String, _ title: String, _ n: Int, _ dot: Color) -> some View {
        Button { filter = key } label: {
            HStack(spacing: 6) {
                Circle().fill(dot).frame(width: 6, height: 6)
                Text(title).foregroundColor(filter == key ? K.text : K.text2)
                Text("\(n)").fontWeight(.medium).foregroundColor(K.dim)
            }
            .font(.system(size: 12.5, weight: .semibold))
            .padding(.horizontal, 11).frame(height: 30)
            .background(filter == key ? K.raised : Color.clear, in: Capsule())
            .overlay(Capsule().strokeBorder(filter == key ? K.raised : K.line, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }
}

// MARK: Не открывается?

private struct FailuresScreen: View {
    @EnvironmentObject var engine: Engine
    let fails: [Failure]
    let list: [Rule]
    let onBack: () -> Void

    var body: some View {
        KScreen {
            BackButton(label: "Соединение", action: onBack)
            SubHead(title: "Не открывается?", lead: "Недавние неудачные соединения. Если сайт не открывается напрямую, пустите его через VPN — и наоборот.")
            if fails.isEmpty {
                CxEmpty(text: "Неудачных соединений нет")
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(fails.enumerated()), id: \.offset) { i, f in
                        if i > 0 { CxLine() }
                        let target = Live.target(f.host, engine.catalog)
                        let vpn = Live.rule(list, f.host, engine.catalog).map { $0.route == "vpn" } ?? (f.route == "vpn")
                        RuleRow(label: f.host, title: f.host, sub: "\(Rules.routeName(f.route)) · \(reason(f.error)) · \(ago(f.at))") {
                            KToggle(isOn: Binding(get: { vpn }, set: { v in
                                Task { await engine.routeTarget(target, vpn: v, label: Rules.title(target, engine.catalog)) }
                            }))
                        }
                    }
                }
                .kCard(14)
                .padding(.top, 14)
            }
            FootNote(text: "Причина обрыва известна не всегда. Список хранится только в памяти и пропадает при выходе.")
        }
    }

    private func ago(_ at: Double) -> String {
        let m = max(0, Int(((Date().timeIntervalSince1970 - at) / 60).rounded()))
        return m < 1 ? "только что" : m < 60 ? "\(m) мин назад" : "\(Int((Double(m) / 60).rounded())) ч назад"
    }

    /// i18n.ts: failureReason.
    private func reason(_ error: String) -> String {
        let e = error.lowercased()
        if e.contains("timeout") || e.contains("deadline") { return "нет ответа" }
        if e.contains("reset") { return "сброс соединения" }
        if e.contains("refused") { return "отказ сервера" }
        if e.contains("no such host") || e.contains("dns") { return "имя не найдено" }
        if e.contains("eof") || e.contains("closed") { return "обрыв" }
        if e.contains("unreachable") { return "адрес недоступен" }
        return "ошибка соединения"
    }
}

// MARK: Схема

/// Живая схема: устройство → через VPN или напрямую → интернет; под ней — кто идёт по каждой линии.
private struct LiveSchema: View {
    let running: Bool
    let server: String?
    let conns: [LiveConn]
    let catalog: [Service]

    var body: some View {
        let vpn = lane("vpn"), direct = lane("direct"), block = lane("block")
        let vpnOn = running && !vpn.names.isEmpty
        let directOn = !running || !direct.names.isEmpty
        VStack(spacing: 0) {
            GeometryReader { g in
                let w = g.size.width, h = g.size.height
                ZStack {
                    FlowPath(up: true, active: vpnOn, color: K.accent, speed: 0.7).frame(width: w, height: h)
                    FlowPath(up: false, active: directOn, color: K.dim, speed: 0.9).frame(width: w, height: h)
                    node(title: "Устройство", sub: "приложения") {
                        VStack(spacing: 2) {
                            RoundedRectangle(cornerRadius: 3).strokeBorder(K.text2, lineWidth: 2).frame(width: 20, height: 14)
                            RoundedRectangle(cornerRadius: 1).fill(K.text2).frame(width: 10, height: 2)
                        }
                    }
                    .position(x: w * 0.14, y: h * 0.5)
                    chip(running ? server ?? "VPN" : "VPN выключен", live: running).position(x: w * 0.5, y: h * 0.24)
                    plainChip("Напрямую").position(x: w * 0.5, y: h * 0.76)
                    node(title: "Интернет", sub: "сайты") { KIcon("globe", size: 20).foregroundColor(K.text2) }
                        .position(x: w * 0.86, y: h * 0.5)
                }
            }
            .frame(height: 150)
            VStack(alignment: .leading, spacing: 6) {
                if running {
                    laneText(K.accent, "через VPN", vpn)
                    laneText(K.dim, "напрямую", direct)
                    if !block.names.isEmpty { laneText(K.red, "блок", block) }
                } else {
                    Text("VPN выключен: всё идёт напрямую. Схема оживёт после подключения.").font(.system(size: 12)).foregroundColor(K.dim)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 6).padding(.top, 4).padding(.bottom, 6)
        }
        .padding(8)
        .kCard(16)
        .padding(.top, 10)
    }

    /// Кто идёт по линии: самые активные — сверху, остальные — «+N».
    private func lane(_ route: String) -> (names: [String], more: Int) {
        var bytes: [String: Double] = [:]
        for c in conns where c.route == route {
            let name = Live.service(for: c.host, catalog)?.name ?? (c.host.hasPrefix("www.") ? String(c.host.dropFirst(4)) : c.host)
            bytes[name, default: 0] += c.download + c.upload
        }
        let sorted = bytes.sorted { $0.value > $1.value }.map(\.key)
        return (Array(sorted.prefix(3)), max(0, sorted.count - 3))
    }

    private func laneText(_ c: Color, _ title: String, _ l: (names: [String], more: Int)) -> some View {
        HStack(alignment: .top, spacing: 8) {
            Circle().fill(c).frame(width: 8, height: 8).padding(.top, 4)
            (Text(title).fontWeight(.semibold).foregroundColor(K.text)
                + Text(" — " + (l.names.isEmpty ? "пока никого" : l.names.joined(separator: ", ") + (l.more > 0 ? " +\(l.more)" : ""))))
                .font(.system(size: 12)).foregroundColor(K.text3)
        }
    }

    private func node<Icon: View>(title: String, sub: String, @ViewBuilder icon: () -> Icon) -> some View {
        VStack(spacing: 0) {
            icon().frame(width: 40, height: 40).background(K.elem, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
            Text(title).font(.system(size: 12, weight: .semibold)).foregroundColor(K.text).padding(.top, 6)
            Text(sub).font(.system(size: 11)).foregroundColor(K.dim)
        }
        .frame(width: 72)
    }

    private func chip(_ text: String, live: Bool) -> some View {
        HStack(spacing: 6) {
            Circle().fill(live ? K.accent : Color.clear).overlay(Circle().strokeBorder(live ? Color.clear : K.ring, lineWidth: 1.5)).frame(width: 6, height: 6)
            Text(text).font(.system(size: 12, weight: .semibold)).foregroundColor(live ? K.text : K.dim).lineLimit(1)
        }
        .padding(.horizontal, 10).frame(height: 28)
        .background(live ? K.accent.opacity(0.16) : K.win, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous)
            .strokeBorder(live ? K.accent.opacity(0.4) : K.ring, style: StrokeStyle(lineWidth: 1, dash: live ? [] : [3, 3])))
        .frame(maxWidth: 190)
    }

    private func plainChip(_ text: String) -> some View {
        Text(text).font(.system(size: 12, weight: .semibold)).foregroundColor(K.text)
            .padding(.horizontal, 10).frame(height: 28)
            .background(K.win, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 9, style: .continuous).strokeBorder(K.line, lineWidth: 1))
    }
}

/// Линия схемы: кривая от устройства к интернету через верх (VPN) или низ (напрямую); идёт трафик — бегут штрихи.
private struct FlowPath: View {
    let up: Bool
    let active: Bool
    let color: Color
    let speed: Double

    var body: some View {
        TimelineView(.animation(paused: !active)) { ctx in
            let phase = active ? CGFloat(ctx.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: speed) / speed) * -16 : 0
            Canvas { g, size in
                let sx = size.width / 100, sy = size.height / 100, y = up ? 24.0 : 76.0
                var p = Path()
                p.move(to: CGPoint(x: 14 * sx, y: 50 * sy))
                p.addCurve(to: CGPoint(x: 50 * sx, y: y * sy), control1: CGPoint(x: 32 * sx, y: 50 * sy), control2: CGPoint(x: 30 * sx, y: y * sy))
                p.addCurve(to: CGPoint(x: 86 * sx, y: 50 * sy), control1: CGPoint(x: 70 * sx, y: y * sy), control2: CGPoint(x: 68 * sx, y: 50 * sy))
                g.stroke(p, with: .color(active ? color : K.ring.opacity(0.8)),
                         style: StrokeStyle(lineWidth: active ? 2 : 1.5, lineCap: .round, dash: active ? [3, 5] : [2, 5], dashPhase: phase))
            }
        }
    }
}

// MARK: Как вас видят сайты

/// «Как вас видят сайты»: адрес, страна и провайдер через VPN и напрямую, утечки IPv6 и DNS. Проверка — по кнопке.
private struct IpCard: View {
    @EnvironmentObject var engine: Engine
    /// Последний отчёт живёт, пока открыто приложение.
    private static var last: IpReport?
    @State private var report: IpReport? = IpCard.last
    @State private var busy = false

    var body: some View {
        KSectionHead(title: "Как вас видят сайты") {
            if report != nil {
                Button { Task { await check() } } label: {
                    HStack(spacing: 5) {
                        KIcon("refresh", size: 14)
                        Text(busy ? "Проверяю…" : "Проверить")
                    }
                    .font(.system(size: 13, weight: .medium)).foregroundColor(K.text2)
                }
                .buttonStyle(.plain)
                .disabled(busy)
            }
        }
        VStack(alignment: .leading, spacing: 0) {
            if let r = report {
                grid(r)
                Text("Город примерный: базы геолокации расходятся. «Похоже на VPN» — по базе одного сервиса, у других может отличаться. WebRTC — только в браузере. Проверено в \(time(r.checkedAt)).")
                    .font(.system(size: 12)).foregroundColor(K.dim2).lineSpacing(2).fixedSize(horizontal: false, vertical: true).padding(.top, 12)
            } else {
                Text("Какой адрес, страну и провайдера видят сайты — через VPN и напрямую. Заодно проверим, не уходят ли IPv6 и DNS мимо туннеля.")
                    .font(.system(size: 13)).foregroundColor(K.text3).lineSpacing(2).fixedSize(horizontal: false, vertical: true)
                Button { Task { await check() } } label: {
                    HStack(spacing: 8) {
                        KIcon("refresh", size: 18)
                        Text(busy ? "Проверяю…" : "Проверить")
                    }
                }
                .buttonStyle(CardButtonStyle())
                .disabled(busy)
                .padding(.top, 12)
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .kCard()
        .padding(.top, 10)
    }

    private func check() async {
        guard !busy else { return }
        busy = true
        if let r = await engine.ipCheck() {
            report = r
            IpCard.last = r
        }
        busy = false
    }

    private struct Cell {
        var text: String
        var color: Color = K.text
        var mono = false
    }

    private func cells(_ c: IpColumn?, vpnColumn: Bool) -> [Cell] {
        let dash = Cell(text: "—", color: K.dim2)
        guard let c else { return [Cell(text: "VPN выключен", color: K.dim2)] + Array(repeating: dash, count: 6) }
        if c.error != nil && c.ipv4 == nil && c.ipv6 == nil { return [Cell(text: "не удалось", color: K.red)] + Array(repeating: dash, count: 6) }
        let country = c.countryCode.map { Countries.name($0.uppercased()) } ?? c.country
        let detected: Cell = c.vpnDetected == nil ? dash
            : c.vpnDetected! ? Cell(text: "да", color: vpnColumn ? K.orange : K.red) : Cell(text: "нет", color: vpnColumn ? K.accent : K.text)
        return [
            Cell(text: c.ipv4 ?? "—", mono: true),
            c.ipv6.map { Cell(text: $0, mono: true) } ?? Cell(text: "нет", color: K.dim2),
            Cell(text: country ?? "—"),
            Cell(text: c.city ?? "—"),
            Cell(text: c.provider ?? "—"),
            c.reverseDns.map { Cell(text: $0, mono: true) } ?? dash,
            detected,
        ]
    }

    private func grid(_ r: IpReport) -> some View {
        let labels = ["IP", "IPv6", "Страна", "Город, примерно", "Провайдер", "Имя адреса", "Похоже на VPN"]
        let a = cells(r.viaVpn, vpnColumn: true), b = cells(r.direct, vpnColumn: false)
        return VStack(spacing: 8) {
            HStack {
                Text("").frame(width: 92, alignment: .leading)
                Text("ЧЕРЕЗ VPN").tracking(0.5).frame(maxWidth: .infinity, alignment: .leading)
                Text("НАПРЯМУЮ").tracking(0.5).frame(maxWidth: .infinity, alignment: .leading)
            }
            .font(.system(size: 11)).foregroundColor(K.dim)
            ForEach(labels.indices, id: \.self) { i in
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Text(labels[i]).foregroundColor(K.dim).frame(width: 92, alignment: .leading)
                    value(a[i])
                    value(b[i])
                }
            }
            Rectangle().fill(K.line).frame(height: 1).padding(.vertical, 2)
            flag("Утечка IPv6", r.ipv6Leak, bad: true)
            flag("DNS через kl!ck", r.dnsProtected, bad: false)
        }
        .font(.system(size: 12))
    }

    private func value(_ c: Cell) -> some View {
        Text(c.text).font(c.mono ? .system(size: 11.5, design: .monospaced) : .system(size: 12))
            .foregroundColor(c.color).lineLimit(1).truncationMode(.tail)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Утечка: «есть» — плохо; DNS: «да» — хорошо.
    private func flag(_ label: String, _ v: Bool?, bad: Bool) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            Text(label).foregroundColor(K.dim).frame(width: 92, alignment: .leading)
            Text(v == nil ? "—" : bad ? (v! ? "есть" : "нет") : (v! ? "да" : "нет"))
                .foregroundColor(v == nil ? K.dim2 : (v! == bad) ? K.red : K.accent)
                .frame(maxWidth: .infinity, alignment: .leading)
            Spacer().frame(maxWidth: .infinity)
        }
    }

    private func time(_ unix: Double) -> String {
        let f = DateFormatter()
        f.dateFormat = "HH:mm"
        return f.string(from: Date(timeIntervalSince1970: unix))
    }
}

// MARK: Назад

/// `.back`: «‹ Назад» над подэкраном.
struct BackButton: View {
    let label: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                KIcon("chevron", size: 14).rotationEffect(.degrees(180))
                Text(label).font(.system(size: 13, weight: .medium))
            }
            .foregroundColor(K.text2)
            .padding(.leading, 8).padding(.trailing, 14)
            .frame(height: 32)
            .kSurface(Capsule(), interactive: true)
        }
        .buttonStyle(Press())
        .padding(.top, 14)
    }
}
