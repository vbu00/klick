import SwiftUI

/// «Соединение» — как ui/src/screens/Connection.tsx на iOS: куда направлять, готовые наборы и мой список.
/// Добавление в список, «Сейчас в сети» и схема — пока в обычном kl!ck.
struct ConnectionScreen: View {
    @EnvironmentObject var engine: Engine
    @State private var catalog: [Service] = []
    @State private var showList = false

    var body: some View {
        if showList {
            RulesList(catalog: catalog) { showList = false }
        } else if let s = engine.settings {
            main(s)
        }
    }

    private func main(_ s: Settings) -> some View {
        let list = s.routing == "all_vpn" ? s.lists?.allVpn ?? [] : s.lists?.selected ?? []
        return KScreen {
            KTitle("Соединение", xl: true)

            CxHead("Куда направлять")
            KSeg(value: Binding(get: { s.routing }, set: { r in Task { await engine.setRouting(r) } }),
                 options: [("all_vpn", "VPN для всего"), ("selected", "VPN для выбранного")], height: 38, size: 14, weight: .semibold)
            VStack(spacing: 0) {
                if s.routing == "selected" {
                    preset("Заблокированное в РФ — через VPN", "Готовый набор, обновляется сам. Остальное — напрямую.",
                           on: Binding(get: { s.blockedPreset != false }, set: { v in Task { await engine.setPrefs(["blocked_preset": v]) } }))
                } else {
                    preset(".ru и .рф напрямую", "Госуслуги, банки, Яндекс — без VPN",
                           on: Binding(get: { s.russiaDirect?.domains ?? false }, set: { v in Task { await engine.setPref("ru_domains", v) } }))
                    Rectangle().fill(K.line).frame(height: 1)
                    preset("Российские IP напрямую", "По базе стран — кроме заблокированного в РФ: оно всё равно через VPN",
                           on: Binding(get: { s.russiaDirect?.ips ?? false }, set: { v in Task { await engine.setPref("ru_ips", v) } }))
                    Rectangle().fill(K.line).frame(height: 1)
                    preset("Локальная сеть — напрямую", "Роутер, принтер, игры по локалке", on: nil)
                }
            }
            .kCard(14)
            .padding(.top, 8)

            CxHead("Мой список", wide: true)
            Button {
                showList = true
            } label: {
                HStack(spacing: 14) {
                    VStack(alignment: .leading, spacing: 0) {
                        Text("\(s.routing == "selected" ? "Через VPN" : "Напрямую") · мой список")
                            .font(.system(size: 16, weight: .semibold)).foregroundColor(K.text)
                        Text(list.isEmpty ? "Пока пусто — добавьте сервис или сайт" : summary(list))
                            .font(.system(size: 12.5)).foregroundColor(K.dim).padding(.top, 3)
                        if !list.isEmpty {
                            HStack(spacing: -6) {
                                ForEach(Array(list.prefix(6).enumerated()), id: \.offset) { _, r in
                                    RuleTile(label: title(r), size: 28)
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
            .buttonStyle(.plain)

            Text("Добавлять сервисы и сайты в список, смотреть «Сейчас в сети» и схему пока можно в обычном kl!ck.")
                .font(.system(size: 12.5)).foregroundColor(K.dim).lineSpacing(2)
                .padding(.horizontal, 4).padding(.top, 8)
        }
        .task { if catalog.isEmpty, let c = try? await engine.get([Service].self, "catalog") { catalog = c } }
    }

    private func preset(_ title: String, _ desc: String, on: Binding<Bool>?) -> some View {
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

    private func title(_ r: Rule) -> String { Rules.title(r, catalog) }

    private func summary(_ list: [Rule]) -> String {
        let services = list.filter { $0.target.kind == "service" }.count
        let sites = list.filter { ["domain", "ip"].contains($0.target.kind) }.count
        return [services > 0 ? Texts.plural(services, ("сервис", "сервиса", "сервисов")) : nil,
                sites > 0 ? Texts.plural(sites, ("сайт", "сайта", "сайтов")) : nil].compactMap { $0 }.joined(separator: " · ")
    }
}

/// `.cx-head`: подпись раздела «Соединения» — 12, полужирный, заглавными.
private struct CxHead: View {
    let text: String
    var wide = false

    init(_ text: String, wide: Bool = false) {
        self.text = text
        self.wide = wide
    }

    var body: some View {
        Text(text.uppercased()).font(.system(size: 12, weight: .semibold)).tracking(0.72).foregroundColor(K.dim)
            .padding(.horizontal, 4).padding(.top, wide ? 22 : 20).padding(.bottom, 8)
    }
}

/// Сервис каталога (`catalog`): нужно только имя.
struct Service: Decodable {
    var id: String
    var name: String
}

/// Названия и цвета правил — как ui/src/lib/rules.ts.
enum Rules {
    private static let tints: [UInt32] = [0xa8c7fa, 0xf5c26b, 0xc4b5fd, 0x9ad9c0, 0xf4a7b9, 0xb8c4d0]

    static func tint(_ key: String) -> Color {
        var h: UInt32 = 0
        for ch in key.utf16 { h = h &* 31 &+ UInt32(ch) }
        return Color(hex: tints[Int(h % UInt32(tints.count))])
    }

    static func title(_ r: Rule, _ catalog: [Service]) -> String {
        let v = r.target.value
        switch r.target.kind {
        case "service": return catalog.first { $0.id == v }?.name ?? v
        case "domain": return v.contains(".") ? v : ".\(v)"
        case "ip": return v.replacingOccurrences(of: "/32", with: "").replacingOccurrences(of: "/128", with: "")
        default: return v
        }
    }
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

/// Мой список: правила текущего положения тумблера; выключенные — бледнее.
private struct RulesList: View {
    @EnvironmentObject var engine: Engine
    let catalog: [Service]
    let onBack: () -> Void

    var body: some View {
        let s = engine.settings
        let routing = s?.routing ?? "selected"
        let list = routing == "all_vpn" ? s?.lists?.allVpn ?? [] : s?.lists?.selected ?? []
        KScreen {
            BackButton(label: "Соединение", action: onBack)
            KTitle(routing == "selected" ? "Через VPN" : "Напрямую", xl: true)
            if list.isEmpty {
                Text("Пока пусто — добавьте сервис или сайт в обычном kl!ck.")
                    .font(.system(size: 13)).foregroundColor(K.dim).multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity).padding(22).kCard(14).padding(.top, 14)
            } else {
                VStack(spacing: 0) {
                    ForEach(Array(list.enumerated()), id: \.offset) { i, r in
                        if i > 0 { Rectangle().fill(K.line).frame(height: 1) }
                        HStack(spacing: 12) {
                            RuleTile(label: Rules.title(r, catalog)).opacity(r.enabled == false ? 0.45 : 1)
                            VStack(alignment: .leading, spacing: 0) {
                                Text(Rules.title(r, catalog)).font(.system(size: 14, weight: .semibold))
                                    .foregroundColor(r.enabled == false ? K.dim : K.text).lineLimit(1)
                                Text(kind(r.target.kind)).font(.system(size: 11.5)).foregroundColor(K.dim)
                            }
                            Spacer(minLength: 0)
                        }
                        .padding(.leading, 14).padding(.trailing, 10).padding(.vertical, 11)
                    }
                }
                .kCard(14)
                .padding(.top, 14)
            }
        }
    }

    private func kind(_ k: String) -> String {
        switch k {
        case "service": return "сервис"
        case "domain": return "сайт"
        case "ip": return "IP-адрес"
        default: return k
        }
    }
}

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
