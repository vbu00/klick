import SwiftUI
import UIKit

/// Главная — как ui/src/screens/Home.tsx: статус, кнопка, чип режима, подписки с серверами, скорость.
/// Подписку можно посмотреть, не переключаясь на неё; активной она становится, когда в ней включают VPN
/// или выбирают сервер.
struct HomeView: View {
    @EnvironmentObject var engine: Engine
    @Binding var tab: Int
    @State private var viewId: String?
    /// Серверы раскрыты; `KLICK_EXPAND=1` — для снимков в симуляторе.
    @State private var expanded = ProcessInfo.processInfo.environment["KLICK_EXPAND"] == "1"
    @State private var switchTo: String?
    /// Сервер, выбранный в другой подписке под VPN: поставить его после переключения.
    @State private var switchServer: String?

    private var conns: [Connection] { engine.settings?.connections ?? [] }
    private var activeId: String? { engine.activeId }
    private var liveId: String? { engine.isRunning ? activeId : nil }
    private var view: Connection? { conns.first { $0.id == (viewId ?? activeId) } ?? conns.first }
    private var vpn: String { engine.state?.vpn ?? "off" }

    var body: some View {
        if conns.isEmpty {
            EmptyHome { tab = Tab.add.rawValue }
        } else if let view {
            screen(view)
        }
    }

    private func screen(_ view: Connection) -> some View {
        let isActiveView = view.id == activeId
        let viewingLive = engine.isRunning && view.id == liveId
        let list = engine.servers[view.id] ?? []
        // Пока сервер не выбран, ядро при подключении берёт первый — его и показываем выбранным.
        let current = list.first(where: \.selected) ?? list.first { $0.name == view.selectedServer } ?? list.first
        return KScreen {
            VStack(spacing: 2) {
                Text(statusLabel(viewingLive)).font(.system(size: 15)).foregroundColor(statusTone)
                TimelineView(.periodic(from: .now, by: 1)) { ctx in
                    let timer = Texts.timer(since: engine.isRunning ? engine.state?.since : nil, now: ctx.date)
                    Text(timer)
                        .font(.system(size: 30, weight: .medium)).tracking(0.5).monospacedDigit().foregroundColor(K.text)
                        .kNumeric(timer)
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.top, 28)
            .zIndex(2)

            PowerButton(core: core(viewingLive), wave: viewingLive && vpn == "connected" && engine.pending == nil) {
                Haptics.tap()
                Task { await onPower(view, isActiveView: isActiveView, viewingLive: viewingLive) }
            }
            .padding(.top, 22)
            .zIndex(1)

            Button {
                tab = Tab.connection.rawValue
            } label: {
                HStack(spacing: 8) {
                    Circle().fill(modeDot).frame(width: 7, height: 7)
                    Text(engine.state?.routing == "all_vpn" ? "VPN для всего" : "VPN для выбранного")
                        .font(.system(size: 13)).foregroundColor(K.text2)
                }
                .modifier(ChipLook())
            }
            .kButton(.chip)
            .frame(maxWidth: .infinity)
            .padding(.top, 18)

            banner

            if conns.count > 1 {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 6) {
                        ForEach(conns) { c in
                            PChip(title: c.name, on: c.id == view.id, live: engine.isRunning && c.id == liveId) {
                                Haptics.pick()
                                expanded = false
                                withAnimation(.kSpring) { viewId = c.id }
                            }
                        }
                    }
                    .padding(1)
                }
                .padding(.top, 34)
            }

            ProfileCard(conn: view, viewingLive: viewingLive, live: vpn == "connected", isActive: isActiveView, list: list, current: current,
                        expanded: $expanded, pinging: engine.pinging && isActiveView,
                        onPickOther: { name in Task { await pickOther(view, name) } })
                .padding(.top, conns.count > 1 ? 10 : 34)

            SpeedBlock()
        }
        .refreshable {
            if view.kind == "subscription" { await engine.refresh(view.id) } else { await engine.reload() }
        }
        .confirmationDialog("Сменить подключение?", isPresented: Binding(get: { switchTo != nil }, set: { if !$0 { switchTo = nil; switchServer = nil } }), titleVisibility: .visible) {
            Button("Переключить") {
                guard let id = switchTo else { return }
                let server = switchServer
                switchTo = nil
                switchServer = nil
                Task { if await engine.selectConnection(id), let server { await engine.selectServer(server) } }
            }
            Button("Отмена", role: .cancel) {}
        } message: {
            Text("Сейчас работает \(conns.first { $0.id == liveId }?.name ?? "другое подключение"). Текущее соединение разорвётся на пару секунд, и трафик пойдёт через \(conns.first { $0.id == switchTo }?.name ?? "выбранное").")
        }
        .onChange(of: engine.focusConnection) { id in
            guard let id else { return }
            viewId = id
            expanded = false
            engine.focusConnection = nil
            if vpn == "off", activeId != id { Task { await engine.selectConnection(id) } }
        }
        .onChange(of: viewId) { id in
            if let id, id != activeId, engine.servers[id] == nil { Task { await engine.viewServers(id) } }
        }
    }

    // MARK: Состояние

    private func statusLabel(_ viewingLive: Bool) -> String {
        if vpn == "connecting" || engine.pending == "on" { return "подключаюсь…" }
        if engine.pending == "off" { return "отключаюсь…" }
        switch vpn {
        case "reconnecting":
            return "переподключаюсь… " + attempt
        case "server_down": return "сервер не отвечает"
        case "error": return "VPN не включился"
        default: return !engine.isRunning ? "не подключено" : viewingLive ? "время подключения" : "нажмите, чтобы переключиться сюда"
        }
    }

    /// «2 из 3» — попытка переподключения.
    private var attempt: String {
        let a = engine.state?.attempt ?? []
        return a.count == 2 ? "\(a[0]) из \(a[1])" : ""
    }

    private var statusTone: Color {
        if engine.pending != nil { return K.dim }
        return vpn == "reconnecting" || vpn == "server_down" ? K.orange : vpn == "error" ? K.red : K.dim
    }

    private func core(_ viewingLive: Bool) -> PowerButton.Core {
        if vpn == "connecting" || engine.pending != nil { return .busy }
        switch vpn {
        case "reconnecting": return viewingLive ? .busyWarn : .other
        case "server_down": return viewingLive ? .warn : .other
        case "error": return .bad
        default: return engine.isRunning ? (viewingLive ? .live : .other) : .off
        }
    }

    private var modeDot: Color {
        vpn == "error" ? K.red : vpn == "reconnecting" || vpn == "server_down" ? K.orange : engine.isRunning ? K.accent : K.dim
    }

    @ViewBuilder
    private var banner: some View {
        switch vpn {
        case "reconnecting":
            Banner(title: "Переподключаюсь… " + attempt, sub: "Сервер перестал отвечать, пробую снова")
        case "server_down":
            Button { expanded = true } label: {
                Banner(title: "Сервер не отвечает", sub: "Проверяю раз в минуту. Можно выбрать другой сервер")
            }
            .buttonStyle(.plain)
        case "error":
            Banner(title: "VPN не включился", sub: Texts.error(engine.state?.error ?? ""), bad: true)
        default:
            EmptyView()
        }
    }

    // MARK: Действия

    private func onPower(_ view: Connection, isActiveView: Bool, viewingLive: Bool) async {
        // Второе нажатие, пока движок не ответил на первое, — не команда.
        if vpn == "connecting" || engine.pending != nil { return }
        if engine.isRunning && !viewingLive {
            switchTo = view.id
            return
        }
        await engine.power(viewing: view.id)
    }

    /// Сервер в подписке, которая сейчас не активна: это и есть переключение на неё. Под VPN — с вопросом.
    private func pickOther(_ view: Connection, _ name: String) async {
        if engine.isRunning {
            switchServer = name
            switchTo = view.id
            return
        }
        if await engine.selectConnection(view.id) { await engine.selectServer(name) }
    }
}

extension View {
    /// Долгое нажатие на сервер: проверить задержку (в активной подписке), скопировать имя.
    func serverMenu(_ name: String, active: Bool) -> some View {
        modifier(ServerMenu(name: name, active: active))
    }
}

private struct ServerMenu: ViewModifier {
    @EnvironmentObject var engine: Engine
    let name: String
    let active: Bool

    func body(content: Content) -> some View {
        content.contextMenu {
            if active {
                Button { Task { await engine.testLatency() } } label: { Label("Проверить задержку", image: "server") }
            }
            Button {
                UIPasteboard.general.string = name
                engine.toast = Toast("Имя скопировано", name)
            } label: { Label("Скопировать имя", image: "copy") }
        }
    }
}

/// Нажатие уменьшает на 4 %, как `:active { transform: scale(0.96) }`.
struct Press: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.scaleEffect(configuration.isPressed ? 0.96 : 1).animation(.spring(response: 0.25, dampingFraction: 0.7), value: configuration.isPressed)
    }
}

/// `.mode-chip`: капсула 34 на карточке.
struct ChipLook: ViewModifier {
    func body(content: Content) -> some View {
        // Со стеклом — стеклянная капсула, откликается на касание.
        content.padding(.horizontal, 16).frame(height: 34).kSurface(Capsule(), interactive: true, glassy: true)
    }
}

/// `.banner`: оранжевая (или красная) плашка с точкой.
private struct Banner: View {
    let title: String
    let sub: String
    var bad = false

    var body: some View {
        HStack(spacing: 10) {
            Circle().fill(bad ? K.red : K.orange).frame(width: 7, height: 7)
            VStack(alignment: .leading, spacing: 1) {
                Text(title).font(.system(size: 13)).foregroundColor(K.text)
                Text(sub).font(.system(size: 12)).foregroundColor(K.text3)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 11)
        .background((bad ? K.red : K.orange).opacity(0.12), in: RoundedRectangle(cornerRadius: K.r(14), style: .continuous))
        .background(K.card, in: RoundedRectangle(cornerRadius: K.r(14), style: .continuous))
        .padding(.top, 16)
    }
}

/// `.pchip`: чип подписки; работающая — с пульсирующей точкой.
private struct PChip: View {
    let title: String
    let on: Bool
    let live: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if live { LiveDot() }
                FlagName(name: title, size: 12)
            }
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(on ? K.text : K.dim)
            .padding(.horizontal, 12)
            .frame(height: 30)
            .modifier(PChipLook(on: on))
        }
        .buttonStyle(Press())
    }
}

/// Чип: без стекла — обводка, выбранный залит; со стеклом — стеклянная капсула (выбранная — плотнее).
private struct PChipLook: ViewModifier {
    @Environment(\.klickGlass) private var glass
    let on: Bool

    func body(content: Content) -> some View {
        if glass {
            content.kSurface(Capsule(), tint: on ? K.raised : nil, interactive: true, glassy: true)
        } else {
            content
                .background(on ? K.raised : Color.clear, in: Capsule())
                .overlay(Capsule().strokeBorder(on ? K.raised : K.line, lineWidth: 1))
        }
    }
}

/// Точка 6 с пульсом до 42 (`pulse-dot`).
private struct LiveDot: View {
    @State private var on = false
    var body: some View {
        Circle().fill(K.accent).frame(width: 6, height: 6)
            .background(
                Circle().fill(K.accent.opacity(0.35)).scaleEffect(on ? 7 : 1).opacity(on ? 0 : 1)
            )
            .onAppear { withAnimation(.easeOut(duration: 2.2).repeatForever(autoreverses: false)) { on = true } }
    }
}

// MARK: Кнопка питания

/// `.power-btn` 120 с кругом 84 (`.power-core`) и значком 34; когда VPN работает — волна по точкам вокруг.
struct PowerButton: View {
    enum Core { case off, live, other, busy, busyWarn, warn, bad }

    let core: Core
    let wave: Bool
    let action: () -> Void

    private var fill: Color { core == .live ? K.accent : K.card }
    private var border: Color {
        switch core {
        case .off: return K.line
        case .live, .other: return K.accent
        case .busy: return K.accent.opacity(0.45)
        case .busyWarn, .warn: return K.orange
        case .bad: return K.red
        }
    }
    private var iconColor: Color {
        switch core {
        case .off: return K.dim
        case .live: return K.onAccent
        case .other, .busy: return K.accent
        case .busyWarn, .warn: return K.orange
        case .bad: return K.red
        }
    }

    @Environment(\.klickGlass) private var glass

    private var plainButton: some View {
        Button(action: action) {
            ZStack {
                if core == .live { Pulse() }
                Circle().fill(fill).frame(width: 84, height: 84)
                    .overlay(Circle().strokeBorder(border, lineWidth: 1))
                if core == .busy || core == .busyWarn { Spinner(color: core == .busyWarn ? K.orange : K.accent) }
                KIcon("power", size: 34).foregroundColor(iconColor)
            }
            .frame(width: 120, height: 120)
            .contentShape(Circle())
        }
        .buttonStyle(Press())
        .accessibilityLabel(core == .off ? "Включить VPN" : "Отключить VPN")
    }

    var body: some View {
        ZStack {
            if wave { Wave().transition(.opacity) }
            plainButton
        }
        .frame(maxWidth: .infinity)
        .frame(height: 120)
        .animation(.easeInOut(duration: 0.6), value: wave)
        .animation(.kSpring, value: core)
    }
}

/// `::before` у `.power-core.live`: круг акцента 35 % растёт до 120 и гаснет за 2.2 с.
private struct Pulse: View {
    @State private var on = false
    var body: some View {
        Circle().fill(K.accent.opacity(0.35)).frame(width: 86, height: 86)
            .scaleEffect(on ? 1.4286 : 1).opacity(on ? 0 : 1)
            .onAppear { withAnimation(.easeOut(duration: 2.2).repeatForever(autoreverses: false)) { on = true } }
    }
}

/// `.power-core.busy::after`: дуга сверху по кругу 94, оборот за секунду.
private struct Spinner: View {
    let color: Color
    @State private var turn = false
    var body: some View {
        Circle().trim(from: 0, to: 0.25)
            .stroke(color, style: StrokeStyle(lineWidth: 2, lineCap: .round))
            .frame(width: 92, height: 92)
            .rotationEffect(.degrees(-135 + (turn ? 360 : 0)))
            .onAppear { withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { turn = true } }
    }
}

/// `.wave`: сетка точек 14 вокруг кнопки (гаснет к краям), по ней раз в 5.2 с расходится зелёная волна
/// (30 кадров в секунду — глазу хватает, батарее легче),
/// а от круга — кольцо (`.lit`, `.ringfx`).
private struct Wave: View {
    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30)) { ctx in
            let t = ctx.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: 5.2) / 5.2
            Canvas { g, size in
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                // Маска `radial-gradient(ellipse at center, #000 20%, transparent 72%)`: эллипс до дальнего угла.
                let rx = size.width / 2 * 2.squareRoot(), ry = size.height / 2 * 2.squareRoot()
                // Волна: кольцо `radial-gradient(circle …)` до дальнего угла, растёт от 0 до 3.4 к 55 % и гаснет к 62 %.
                let far = (c.x * c.x + c.y * c.y).squareRoot()
                let scale: Double = 3.4 * ease(min(1, t / 0.55))
                let alpha: Double = t < 0.55 ? 1 : (t < 0.62 ? 1 - (t - 0.55) / 0.07 : 0)
                let step: CGFloat = 14
                let nx = Int(size.width / step / 2) + 1, ny = Int(size.height / step / 2) + 1
                for iy in -ny...ny {
                    for ix in -nx...nx {
                        let x = c.x + CGFloat(ix) * step, y = c.y + CGFloat(iy) * step
                        let dx = (x - c.x) / rx, dy = (y - c.y) / ry
                        let e = Double((dx * dx + dy * dy).squareRoot())
                        let m: Double = e <= 0.2 ? 1 : (e >= 0.72 ? 0 : 1 - (e - 0.2) / 0.52)
                        guard m > 0 else { continue }
                        g.fill(Path(ellipseIn: CGRect(x: x - 1.25, y: y - 1.25, width: 2.5, height: 2.5)), with: .color(K.dots.opacity(m)))
                        guard scale > 0, alpha > 0 else { continue }
                        let r = Double(((x - c.x) * (x - c.x) + (y - c.y) * (y - c.y)).squareRoot() / far) / scale
                        let lit: Double = Self.band(r)
                        guard lit > 0 else { continue }
                        g.fill(Path(ellipseIn: CGRect(x: x - 1.6, y: y - 1.6, width: 3.2, height: 3.2)), with: .color(K.accent.opacity(m * lit * alpha)))
                    }
                }
                // Кольцо от круга: 84, 1.5, акцент 55 %; 0.55 → 3.2 к 55 %, видно с 8 %.
                let ringScale: Double = 0.55 + (3.2 - 0.55) * ease(min(1, t / 0.55))
                let ringAlpha: Double = t < 0.08 ? 0.7 * t / 0.08 : (t < 0.55 ? 0.7 * (1 - (t - 0.08) / 0.47) : 0)
                if ringAlpha > 0 {
                    let rr = 42 * CGFloat(ringScale)
                    var ring = g
                    ring.addFilter(.blur(radius: 1.5))
                    ring.stroke(Path(ellipseIn: CGRect(x: c.x - rr, y: c.y - rr, width: rr * 2, height: rr * 2)),
                                with: .color(K.accent.opacity(0.55 * ringAlpha)), lineWidth: 1.5)
                }
            }
        }
        .padding(.horizontal, -16)
        .frame(height: 360)
        .allowsHitTesting(false)
    }

    /// Яркость точки в кольце волны: `transparent 0 24%, accent 30% 40%, transparent 48%`.
    private static func band(_ r: Double) -> Double {
        if r < 0.24 || r >= 0.48 { return 0 }
        if r < 0.30 { return (r - 0.24) / 0.06 }
        if r <= 0.40 { return 1 }
        return 1 - (r - 0.40) / 0.08
    }

    /// cubic-bezier(0.3, 0.6, 0.4, 1) — приблизительно.
    private func ease(_ x: Double) -> Double { 1 - pow(1 - x, 2.2) }
}

// MARK: Карточка подписки

/// `.pcard`: значок, название и сервер, трафик, срок, «Обновить», ссылки панели; по нажатию — серверы.
private struct ProfileCard: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.openURL) private var openURL
    let conn: Connection
    let viewingLive: Bool
    let live: Bool
    let isActive: Bool
    let list: [Server]
    let current: Server?
    @Binding var expanded: Bool
    let pinging: Bool
    let onPickOther: (String) -> Void
    @State private var refreshing = false
    @State private var confirmRemove = false

    private var isSub: Bool { conn.kind == "subscription" }
    /// «Серверы по странам»: если ни одной страны не узнали, делить нечего — обычный список.
    private var byCountry: Bool {
        engine.settings?.serversByCountry == true && list.contains { Countries.parse($0.name, $0.kind).code != nil }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
            if isSub { traffic }
            if expanded { servers }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .kCard()
        .confirmationDialog(isSub ? "Удалить подписку?" : "Удалить конфигурацию?", isPresented: $confirmRemove, titleVisibility: .visible) {
            Button("Удалить", role: .destructive) { Task { await engine.remove(conn.id) } }
            Button("Отмена", role: .cancel) {}
        }
    }

    private var head: some View {
        HStack(spacing: 14) {
            KIcon(isSub ? "globe" : "rules", size: 24).foregroundColor(K.dim)
                .frame(width: 52, height: 52)
                .background(K.elem, in: RoundedRectangle(cornerRadius: K.r(16), style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 8) {
                    FlagName(name: conn.name, size: 17).font(.system(size: 17, weight: .semibold)).foregroundColor(K.text)
                    if viewingLive && live {
                        Text("Работает").font(.system(size: 11, weight: .semibold)).foregroundColor(K.accent)
                            .padding(.horizontal, 8).frame(height: 20)
                            .background(K.accent.opacity(0.16), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    }
                }
                HStack(spacing: 6) {
                    // По странам: флаг и «Беларусь · Hysteria2» вместо «🇧🇾 BY · HY2».
                    if let cur = byCountry ? current.map({ ($0, Countries.parse($0.name, $0.kind)) }) : nil, let code = cur.1.code {
                        FlagView(code: code, size: 16)
                        Text("\(Countries.name(code)) · \(cur.1.title.isEmpty ? Texts.protocolName(cur.0.kind) : cur.1.title)")
                            .lineLimit(1).foregroundColor(K.dim)
                    } else {
                        Text(current?.name ?? conn.selectedServer ?? (isSub ? "Подписка" : "Одиночная конфигурация"))
                            .lineLimit(1).foregroundColor(K.dim)
                    }
                    // Задержку показываем, только когда её измерили: «нет ответа» до проверки было бы неправдой.
                    if pinging || current?.delay != nil {
                        Circle().fill(K.dim2).frame(width: 3, height: 3)
                        Text(pinging ? "…" : Texts.ping(current?.delay)).monospacedDigit().kNumeric(current?.delay)
                            .foregroundColor(pinging ? K.dim : K.ping(current?.delay))
                    }
                }
                .font(.system(size: 13))
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            Menu {
                if isSub {
                    Button { Task { await refresh() } } label: { Label("Обновить подписку", image: "refresh") }
                }
                if isActive {
                    Button {
                        expanded = true
                        Task { await engine.testLatency() }
                    } label: { Label("Проверить задержку", image: "server") }
                }
                Divider()
                Button(role: .destructive) { confirmRemove = true } label: {
                    Label(isSub ? "Удалить подписку" : "Удалить конфигурацию", image: "trash")
                }
            } label: {
                KIcon("dots", size: 20).foregroundColor(K.dim).frame(width: 32, height: 32).contentShape(Circle())
            }
        }
        .padding(16)
        .contentShape(Rectangle())
        .onTapGesture { toggle() }
    }

    @ViewBuilder
    private var traffic: some View {
        let info = conn.info
        let used = info?.used ?? 0
        let total = info?.total ?? 0
        // Без лимита панель всё равно сообщает, сколько потрачено (total = 0) — показываем без полосы.
        let unlimited = info != nil && total == 0 && used > 0
        let links: [(String, URL)] = [("Поддержка", info?.supportUrl), ("Страница подписки", info?.pageUrl)].compactMap { (title: String, raw: String?) -> (String, URL)? in
            guard let raw, let url = URL(string: raw) else { return nil }
            return (title, url)
        }
        VStack(alignment: .leading, spacing: 0) {
            if total > 0 {
                trafficRow(Texts.traffic(used, total))
                GeometryReader { g in
                    ZStack(alignment: .leading) {
                        RoundedRectangle(cornerRadius: 3).fill(K.elem)
                        RoundedRectangle(cornerRadius: 3).fill(used / total > 0.9 ? K.orange : K.text)
                            .frame(width: g.size.width * min(1, used / total))
                    }
                }
                .frame(height: 5)
                .padding(.top, 8)
            } else if unlimited {
                trafficRow("\(Texts.used(used)) · без лимита")
            }
            HStack {
                expiry(info, hasTraffic: total > 0 || unlimited).font(.system(size: 12)).foregroundColor(K.dim)
                Spacer(minLength: 8)
                Button { Task { await refresh() } } label: {
                    HStack(spacing: 6) {
                        KIcon("refresh", size: 13).rotationEffect(.degrees(refreshing ? 360 : 0))
                            .animation(refreshing ? .linear(duration: 1).repeatForever(autoreverses: false) : .default, value: refreshing)
                        Text(refreshing ? "Обновляем…" : "Обновить")
                    }
                }
                .kButton(.elem)
                .disabled(refreshing)
            }
            .padding(.top, total > 0 || unlimited ? 12 : 0)
            if let announce = info?.announce, !announce.isEmpty {
                Text(announce).font(.system(size: 12)).foregroundColor(K.dim).fixedSize(horizontal: false, vertical: true).padding(.top, 8)
            }
            if !links.isEmpty {
                HStack(spacing: 8) {
                    ForEach(links, id: \.0) { l in
                        Button(l.0) { openURL(l.1) }.kButton(.elem)
                    }
                }
                .padding(.top, 12)
            }
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 16)
    }

    private func trafficRow(_ value: String) -> some View {
        HStack {
            Text("Трафик").foregroundColor(K.dim)
            Spacer()
            Text(value).monospacedDigit().foregroundColor(K.text)
        }
        .font(.system(size: 12))
    }

    private func expiry(_ info: SubInfo?, hasTraffic: Bool) -> Text {
        if let expire = info?.expire {
            let days = Texts.daysLeft(expire)
            return Text("Действует до ") + Text(Texts.date(expire)).foregroundColor(K.text) + Text(" · " + (days == 0 ? "истекает" : "\(days) дн."))
        }
        return Text(hasTraffic ? "Без срока" : "Срок и трафик панель не сообщает")
    }

    private var servers: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                KCaps("Серверы · \(list.count)" + (byCountry ? " · " + Texts.plural(countries, ("страна", "страны", "стран")) : ""))
                Spacer()
                if isActive {
                    Button(pinging ? "Проверяем…" : "Проверить задержку") { Task { await engine.testLatency() } }
                        .kButton(.elem)
                        .disabled(pinging)
                }
            }
            .padding(.horizontal, 8)
            .padding(.bottom, 8)
            if !isActive && !list.isEmpty {
                Text("Выберите сервер — kl!ck перейдёт на это подключение.").font(.system(size: 12)).foregroundColor(K.dim)
                    .padding(.horizontal, 8).padding(.bottom, 8)
            }
            if list.isEmpty {
                Text(isActive || engine.servers[conn.id] != nil ? "Серверов пока нет. Обновите подписку — список придёт от панели." : "Загружаю список серверов…")
                    .font(.system(size: 12)).foregroundColor(K.dim).lineSpacing(2)
                    .padding(.horizontal, 8).padding(.top, 4).padding(.bottom, 8)
            }
            if byCountry {
                ServerCountries(list: list, current: current, pinging: pinging, tested: engine.tested.contains(conn.id), active: isActive) { name in
                    if isActive { Task { await engine.selectServer(name) } } else { onPickOther(name) }
                }
            } else {
                ForEach(list) { s in
                    serverRow(s)
                }
            }
        }
        .padding(.top, 12)
        .padding(.horizontal, 8)
        .padding(.bottom, 8)
        .overlay(alignment: .top) { Rectangle().fill(K.line).frame(height: 1) }
        .transition(.opacity.combined(with: .move(edge: .top)))
    }

    private func serverRow(_ s: Server) -> some View {
        let on = s.name == current?.name
        return Button {
            Haptics.pick()
            if !isActive { onPickOther(s.name) } else if !on { Task { await engine.selectServer(s.name) } }
        } label: {
            HStack(spacing: 12) {
                KRadio(on: on)
                VStack(alignment: .leading, spacing: 0) {
                    Text(s.name).font(.system(size: 14, weight: .medium)).foregroundColor(K.text).lineLimit(1)
                    Text(Texts.protocolName(s.kind)).font(.system(size: 12)).foregroundColor(K.dim)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                if let speed = s.speed { Text(Texts.mbps(speed)).font(.system(size: 11)).foregroundColor(K.dim).monospacedDigit() }
                Text(pinging ? "…" : Texts.ping(s.delay)).font(.system(size: 13)).monospacedDigit().kNumeric(s.delay)
                    .foregroundColor(pinging ? K.dim : K.ping(s.delay))
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 10)
            .background(on ? K.hover : Color.clear, in: RoundedRectangle(cornerRadius: K.r(12), style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .serverMenu(s.name, active: isActive)
        .padding(.bottom, 4)
    }

    private var countries: Int { Set(list.compactMap { Countries.parse($0.name, $0.kind).code }).count }

    private func toggle() {
        Haptics.soft()
        withAnimation(.kSpring) { expanded.toggle() }
        if expanded && isActive && list.count > 1 && list.allSatisfy({ $0.delay == nil }) { Task { await engine.testLatency() } }
        if expanded && !isActive && engine.servers[conn.id] == nil { Task { await engine.viewServers(conn.id) } }
    }

    private func refresh() async {
        refreshing = true
        await engine.refresh(conn.id)
        refreshing = false
    }
}

// MARK: Скорость

/// Замер скорости, плитки «Чтение» и «Загрузка» и график за 60 секунд.
private struct SpeedBlock: View {
    @EnvironmentObject var engine: Engine

    private var connected: Bool { engine.state?.vpn == "connected" && engine.pending == nil }

    var body: some View {
        VStack(spacing: 0) {
            if connected || engine.speed != nil { speedtest.padding(.top, 10) }
            HStack(spacing: 12) {
                tile("Чтение", "download", engine.traffic.down, engine.traffic.downSum)
                tile("Загрузка", "upload", engine.traffic.up, engine.traffic.upSum)
            }
            .padding(.top, 12)
            graph.padding(.top, 12)
        }
    }

    private var speedtest: some View {
        HStack(spacing: 10) {
            VStack(alignment: .leading, spacing: 2) {
                if engine.speedTesting {
                    Text("Меряю скорость через сервер… до полуминуты")
                } else if let r = engine.speed {
                    Text("↓ \(r.down.map(Texts.mbps) ?? "—") · ↑ \(r.up.map(Texts.mbps) ?? "—")")
                        .font(.system(size: 14, weight: .semibold)).monospacedDigit().foregroundColor(K.text)
                    if let server = r.server { Text(server) }
                } else {
                    Text("Сколько реально даёт сервер")
                }
            }
            .font(.system(size: 12)).foregroundColor(K.dim).lineLimit(1)
            .frame(maxWidth: .infinity, alignment: .leading)
            Button { Task { await engine.speedTest() } } label: {
                HStack(spacing: 6) {
                    KIcon("refresh", size: 13)
                    Text(engine.speedTesting ? "Меряю…" : engine.speed != nil ? "Ещё раз" : "Проверить скорость")
                }
            }
            .kButton(.elem)
            .disabled(engine.speedTesting || !connected)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .kCard(18)
    }

    private func tile(_ title: String, _ icon: String, _ rate: Double, _ sum: Double) -> some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack {
                Text(title).font(.system(size: 14))
                Spacer()
                KIcon(icon, size: 22)
            }
            .foregroundColor(K.dim)
            Text(Texts.rate(rate)).font(.system(size: 22, weight: .semibold)).monospacedDigit().foregroundColor(K.text)
                .kNumeric(rate)
                .lineLimit(1).minimumScaleFactor(0.7).padding(.top, 6)
            Text("всего \(Texts.bytes(sum))").font(.system(size: 12)).foregroundColor(K.dim2).padding(.top, 2)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .kCard()
    }

    private var graph: some View {
        let down = engine.traffic.downHist, up = engine.traffic.upHist
        let peak = down.max() ?? 0
        return VStack(spacing: 0) {
            HStack {
                Text("Последние 60 секунд").font(.system(size: 13))
                Spacer()
                HStack(spacing: 12) {
                    legend("Чтение", K.accent)
                    legend("Загрузка", K.dim)
                }
                .font(.system(size: 12))
            }
            .foregroundColor(K.dim)
            SpeedGraph(down: down, up: up).frame(height: 110).padding(.top, 12)
            HStack {
                Text("−60 с")
                Spacer()
                Text("Пик: \(Texts.rate(peak * 1e6 / 8))")
                Spacer()
                Text("сейчас")
            }
            .font(.system(size: 11)).foregroundColor(K.dim2).padding(.top, 6)
        }
        .padding(16)
        .kCard()
    }

    private func legend(_ title: String, _ color: Color) -> some View {
        HStack(spacing: 5) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 8, height: 3)
            Text(title)
        }
    }
}

/// График: линия «Чтение» с заливкой, «Загрузка» тоньше; сетка — низ и пунктир посередине (viewBox 300×90).
private struct SpeedGraph: View {
    let down: [Double]
    let up: [Double]

    var body: some View {
        Canvas { g, size in
            let maxV = max(50, down.max() ?? 0, up.max() ?? 0) * 1.1
            let sx = size.width / 300, sy = size.height / 90
            var base = Path()
            base.move(to: CGPoint(x: 0, y: 89 * sy))
            base.addLine(to: CGPoint(x: size.width, y: 89 * sy))
            g.stroke(base, with: .color(K.elem), lineWidth: 1)
            var mid = Path()
            mid.move(to: CGPoint(x: 0, y: 45 * sy))
            mid.addLine(to: CGPoint(x: size.width, y: 45 * sy))
            g.stroke(mid, with: .color(K.elem), style: StrokeStyle(lineWidth: 1, dash: [3, 4]))
            func line(_ h: [Double]) -> Path {
                var p = Path()
                for (i, v) in h.enumerated() {
                    let pt = CGPoint(x: CGFloat(Double(i) / Double(max(1, h.count - 1)) * 300) * sx, y: CGFloat(88 - v / maxV * 84) * sy)
                    if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
                }
                return p
            }
            let d = line(down)
            var area = d
            area.addLine(to: CGPoint(x: size.width, y: 89 * sy))
            area.addLine(to: CGPoint(x: 0, y: 89 * sy))
            area.closeSubpath()
            g.fill(area, with: .color(K.accent.opacity(0.12)))
            g.stroke(d, with: .color(K.accent), style: StrokeStyle(lineWidth: 2, lineJoin: .round))
            g.stroke(line(up), with: .color(K.dim), style: StrokeStyle(lineWidth: 1.5, lineJoin: .round))
        }
    }
}

// MARK: Пусто

/// Подключений нет: значок, три шага и «Добавить подключение».
private struct EmptyHome: View {
    let onAdd: () -> Void

    var body: some View {
        KScreen {
            VStack(spacing: 0) {
                KIcon("power", size: 38).foregroundColor(K.dim2)
                    .frame(width: 88, height: 88)
                    .kSurface(Circle())
                Text("Нет подключений").font(.system(size: 20, weight: .semibold)).foregroundColor(K.text).padding(.top, 22)
                Text("Добавьте ссылку на подписку или одиночную конфигурацию — и можно включать.")
                    .font(.system(size: 13)).foregroundColor(K.text3).multilineTextAlignment(.center).lineSpacing(2).padding(.top, 6)
                VStack(spacing: 10) {
                    step(1, "Вставьте ссылку", "Подписка (https://…) или vless://, vmess://, trojan://, ss://")
                    step(2, "Выберите, что пускать в VPN", "Весь трафик или только нужные сайты и сервисы")
                    step(3, "Нажмите кнопку питания", "Заблокированные сервисы пойдут через VPN, остальное — напрямую")
                }
                .padding(.top, 26)
                Button("Добавить подключение", action: onAdd).kButton(.primary(true)).padding(.top, 22)
            }
            .padding(.top, 36)
        }
    }

    private func step(_ n: Int, _ title: String, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            Text("\(n)").font(.system(size: 12, weight: .semibold)).foregroundColor(K.text2)
                .frame(width: 26, height: 26).background(K.elem, in: Circle())
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                Text(text).font(.system(size: 12)).foregroundColor(K.text3).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(14)
        .kCard(14)
    }
}
