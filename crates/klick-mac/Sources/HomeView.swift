import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// Главная по прототипу Димы: скорость и «+» сверху, статус и время, кнопка, режим, подписки, карточка с серверами.
struct HomeView: View {
    @EnvironmentObject var engine: Engine
    var onTab: (Tab) -> Void
    @State private var viewId: String?
    @State private var speedOpen = false
    @State private var addOpen = false
    @State private var switchTo: String?
    @State private var switchServer: String?

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                HStack {
                    SpeedPill { speedOpen = true }
                    Spacer()
                    Button { addOpen = true } label: {
                        Image(systemName: "plus").font(.system(size: 15, weight: .semibold)).foregroundStyle(K.text).frame(width: 34, height: 34)
                    }
                    .buttonStyle(PressStyle(scale: 0.92))
                    .glass(Circle(), interactive: true)
                    .help("Добавить подключение")
                }
                .padding(.top, 40)

                if let settings = engine.settings, let state = engine.state {
                    if settings.connections.isEmpty {
                        Empty { addOpen = true }
                    } else {
                        content(settings, state)
                    }
                } else {
                    ProgressView().controlSize(.small).padding(.top, 120)
                }
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 110)
        }
        .scrollIndicators(.never)
        .sheet(isPresented: $speedOpen) { SpeedSheet().environmentObject(engine) }
        .sheet(isPresented: $addOpen) { AddSheet().environmentObject(engine) }
        .onChange(of: engine.focusConnection) { _, id in
            if let id { viewId = id; engine.focusConnection = nil }
        }
        .alert("Сменить подключение?", isPresented: Binding(get: { switchTo != nil }, set: { if !$0 { switchTo = nil; switchServer = nil } })) {
            Button("Отмена", role: .cancel) {}
            Button("Переключить") {
                let (id, server) = (switchTo, switchServer)
                Task {
                    if let id, await engine.selectConnection(id), let server { await engine.selectServer(server) }
                }
            }
        } message: {
            Text("Текущее соединение разорвётся на пару секунд, и трафик пойдёт через «\(engine.settings?.connections.first { $0.id == switchTo }?.name ?? "")».")
        }
    }

    @ViewBuilder
    private func content(_ settings: Settings, _ state: StateView) -> some View {
        let conns = settings.connections
        let view = conns.first { $0.id == (viewId ?? settings.activeConnection) } ?? conns[0]
        let isActive = view.id == settings.activeConnection
        let viewingLive = engine.isRunning && isActive
        let list = engine.servers[view.id] ?? []
        let current = list.first { $0.selected } ?? list.first { $0.name == view.selectedServer } ?? list.first

        StatusBlock(state: state, viewingLive: viewingLive, pending: engine.pending, running: engine.isRunning)
            .padding(.top, 14)

        PowerButton(core: core(state, viewingLive), live: viewingLive && state.vpn == "connected" && engine.pending == nil) {
            if state.vpn == "connecting" || engine.pending != nil { return }
            if engine.isRunning && !viewingLive { switchTo = view.id; return }
            Task { await engine.power(viewing: view.id) }
        }
        .padding(.top, 14)

        Button { onTab(.routes) } label: {
            HStack(spacing: 8) {
                Circle().fill(dot(state)).frame(width: 7, height: 7)
                Text(state.routing == "all_vpn" ? "VPN для всего" : "VPN для выбранного").font(.system(size: 13, weight: .medium)).foregroundStyle(K.text)
            }
            .padding(.horizontal, 16)
            .frame(height: 34)
        }
        .buttonStyle(PressStyle())
        .pill()
        .padding(.top, 14)

        Banner(state: state)

        if conns.count > 1 {
            HStack(spacing: 6) {
                ForEach(conns) { c in
                    Button { viewId = c.id } label: {
                        HStack(spacing: 6) {
                            if engine.isRunning && c.id == settings.activeConnection { Circle().fill(K.ok).frame(width: 6, height: 6) }
                            Text(c.name).font(.system(size: 13, weight: c.id == view.id ? .semibold : .regular)).lineLimit(1)
                                .foregroundStyle(c.id == view.id ? K.text : K.text2)
                        }
                        .padding(.horizontal, 14)
                        .frame(height: 32)
                    }
                    .buttonStyle(PressStyle())
                    .pill(selected: c.id == view.id)
                }
            }
            .padding(.top, 24)
        }

        SubCard(conn: view, isActive: isActive, list: list, current: current) { name in
            if engine.isRunning { switchServer = name; switchTo = view.id; return }
            Task { if await engine.selectConnection(view.id) { await engine.selectServer(name) } }
        }
        .padding(.top, conns.count > 1 ? 12 : 26)
        .task(id: view.id) {
            if !isActive && engine.servers[view.id] == nil { await engine.viewServers(view.id) }
            if isActive && list.count > 1 && list.allSatisfy({ $0.delay == nil }) && !engine.tested.contains(view.id) { await engine.testLatency() }
        }
    }

    private func core(_ st: StateView, _ viewingLive: Bool) -> PowerButton.Core {
        if st.vpn == "connecting" || engine.pending != nil { return .busy }
        switch st.vpn {
        case "reconnecting": return viewingLive ? .busy : .other
        case "server_down": return viewingLive ? .warn : .other
        case "error": return .bad
        default: return engine.isRunning ? (viewingLive ? .live : .other) : .off
        }
    }

    private func dot(_ st: StateView) -> Color {
        switch st.vpn {
        case "error": K.red
        case "reconnecting", "server_down": K.orange
        default: engine.isRunning ? K.ok : K.text3
        }
    }
}

// MARK: Верх

/// Плашка «↓ 48.2 Мб/с | ↑ 2.5 Мб/с»; по нажатию — лист «Скорость».
struct SpeedPill: View {
    @EnvironmentObject var engine: Engine
    var onOpen: () -> Void

    var body: some View {
        Button(action: onOpen) {
            HStack(spacing: 10) {
                Label { Text(rate(engine.traffic.down)).monospacedDigit() } icon: { Image(systemName: "arrow.down.circle") }
                Rectangle().fill(K.line).frame(width: 1, height: 16)
                Label { Text(rate(engine.traffic.up)).monospacedDigit() } icon: { Image(systemName: "arrow.up.circle") }
            }
            .labelStyle(PillLabel())
            .padding(.horizontal, 12)
            .frame(height: 34)
        }
        .buttonStyle(PressStyle(scale: 0.97))
        .glass(Capsule(), interactive: true)
        .help("Скорость")
    }
}

private struct PillLabel: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 5) {
            configuration.icon.font(.system(size: 14)).foregroundStyle(K.text2)
            configuration.title.font(.system(size: 13, weight: .medium)).foregroundStyle(K.text).frame(minWidth: 62, alignment: .leading)
        }
    }
}

/// Скорость одним числом: «48.2 Мб/с», «512 Кб/с».
func rate(_ bytesPerSec: Double) -> String {
    let mbit = bytesPerSec * 8 / 1e6
    if mbit < 1 { return "\(Int((mbit * 1000).rounded())) Кб/с" }
    return mbit < 100 ? String(format: "%.1f Мб/с", mbit) : "\(Int(mbit.rounded())) Мб/с"
}

/// Объём как в прототипе: «64,8 ГБ», «512 МБ» (двоичные, как у панелей подписок).
func gb(_ bytes: Double) -> String {
    let g = 1_073_741_824.0
    if bytes < g { return "\(Int((bytes / 1_048_576).rounded())) МБ" }
    let v = bytes / g
    return (v < 100 ? String(format: "%.1f", v) : "\(Int(v.rounded()))").replacingOccurrences(of: ".", with: ",") + " ГБ"
}

struct StatusBlock: View {
    var state: StateView
    var viewingLive: Bool
    var pending: String?
    var running: Bool

    var body: some View {
        VStack(spacing: 2) {
            Text(label).font(.system(size: 14)).foregroundStyle(tone)
            TimelineView(.periodic(from: .now, by: 1)) { ctx in
                Text(Texts.timer(since: running ? state.since : nil, now: ctx.date))
                    .font(.system(size: 30, weight: .medium)).monospacedDigit().foregroundStyle(K.text)
            }
        }
    }

    private var label: String {
        if state.vpn == "connecting" || pending == "on" { return "подключаюсь…" }
        if pending == "off" { return "отключаюсь…" }
        switch state.vpn {
        case "reconnecting": return "переподключаюсь…"
        case "server_down": return "сервер не отвечает"
        case "error": return "VPN не включился"
        default: return !running ? "не подключено" : viewingLive ? "время подключения" : "нажмите, чтобы переключиться сюда"
        }
    }

    private var tone: Color {
        switch state.vpn {
        case "reconnecting", "server_down": K.orange
        case "error": K.red
        default: K.text2
        }
    }
}

// MARK: Кнопка питания

/// Кнопка: выключена — стекло; включена — стекло цвета акцента с ореолом и полем точек; занята — бегущее кольцо.
struct PowerButton: View {
    enum Core { case off, live, other, busy, warn, bad }
    var core: Core
    var live: Bool
    var action: () -> Void
    @State private var spin = false
    @State private var halo = false

    var body: some View {
        ZStack {
            DotField(active: live).frame(width: 340, height: 200).allowsHitTesting(false)
            if live {
                Circle().fill(K.accent.opacity(0.28)).frame(width: 92, height: 92)
                    .scaleEffect(halo ? 1.45 : 1).opacity(halo ? 0 : 1)
                    .onAppear { withAnimation(.easeOut(duration: 2.2).repeatForever(autoreverses: false)) { halo = true } }
            }
            Button(action: action) {
                Image(systemName: "power").font(.system(size: 32, weight: .medium))
                    .foregroundStyle(core == .live ? Color.white : color)
                    .frame(width: 92, height: 92)
                    // Включена — залита акцентом, стекло поверх даёт блик и объём (одного оттенка стекла на
                    // светлом фоне не видно).
                    .background { if core == .live { Circle().fill(K.accent) } }
            }
            .buttonStyle(PressStyle())
            .glass(Circle(), tint: core == .live ? K.accent : nil, interactive: true)
            .overlay {
                if core == .busy || core == .warn {
                    Circle().trim(from: 0, to: 0.22).stroke(color, style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                        .frame(width: 102, height: 102)
                        .rotationEffect(.degrees(spin ? 360 : 0))
                        .onAppear { withAnimation(.linear(duration: 1).repeatForever(autoreverses: false)) { spin = true } }
                } else if core != .off && core != .live {
                    Circle().stroke(color, lineWidth: 1.5).frame(width: 92, height: 92)
                }
            }
            .shadow(color: core == .live ? K.accent.opacity(0.4) : .black.opacity(0.1), radius: 14, y: 6)
            .help(core == .off ? "Включить VPN" : "Отключить VPN")
        }
        .frame(height: 140)
    }

    private var color: Color {
        switch core {
        case .off: K.text2
        case .warn: K.orange
        case .bad: K.red
        default: K.accent
        }
    }
}

/// Поле точек вокруг кнопки; пока VPN работает, по нему расходится волна цвета акцента.
struct DotField: View {
    var active: Bool

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30, paused: !active)) { ctx in
            Canvas { g, size in
                let t = ctx.date.timeIntervalSinceReferenceDate
                let c = CGPoint(x: size.width / 2, y: size.height / 2)
                let step: CGFloat = 14
                let maxR = hypot(size.width, size.height) / 2
                let wave = CGFloat((t.truncatingRemainder(dividingBy: 5.2)) / 5.2) * maxR
                var y: CGFloat = step / 2
                while y < size.height {
                    var x: CGFloat = step / 2
                    while x < size.width {
                        let d = hypot(x - c.x, y - c.y)
                        let fade = max(0, 1 - d / (maxR * 0.85))
                        if fade > 0, d > 50 {
                            let near = active ? max(0, 1 - abs(d - wave) / 26) : 0
                            let r: CGFloat = 1.1 + near * 0.7
                            let color = near > 0.05 ? K.accent.opacity(Double(fade * near)) : K.text3.opacity(Double(fade) * 0.45)
                            g.fill(Path(ellipseIn: CGRect(x: x - r, y: y - r, width: r * 2, height: r * 2)), with: .color(color))
                        }
                        x += step
                    }
                    y += step
                }
            }
        }
    }
}

struct Banner: View {
    var state: StateView

    var body: some View {
        if let (title, sub, tone) = info {
            HStack(alignment: .top, spacing: 10) {
                Circle().fill(tone).frame(width: 7, height: 7).padding(.top, 5)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(.system(size: 13.5, weight: .medium)).foregroundStyle(K.text)
                    Text(sub).font(.system(size: 12)).foregroundStyle(K.text2)
                }
                Spacer(minLength: 0)
            }
            .padding(12)
            .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(tone.opacity(0.1)))
            .padding(.top, 14)
        }
    }

    private var info: (String, String, Color)? {
        switch state.vpn {
        case "reconnecting":
            let a = state.attempt.map { " \($0.first ?? 0) из \($0.last ?? 0)" } ?? ""
            return ("Переподключаюсь…\(a)", "Сервер перестал отвечать, пробую снова", K.orange)
        case "server_down": return ("Сервер не отвечает", "Проверяю раз в минуту. Можно выбрать другой сервер ниже", K.orange)
        case "error": return ("VPN не включился", Texts.error(state.error ?? ""), K.red)
        default: return nil
        }
    }
}

// MARK: Карточка подписки

struct SubCard: View {
    @EnvironmentObject var engine: Engine
    var conn: Connection
    var isActive: Bool
    var list: [Server]
    var current: Server?
    var onPickOther: (String) -> Void
    @State private var refreshing = false
    @State private var confirmRemove = false

    private var isSub: Bool { conn.kind == "subscription" }
    private var pinging: Bool { engine.pinging && isActive }

    var body: some View {
        let parsed = current.map { Countries.parse($0.name, $0.kind) }
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 12) {
                FlagBadge(code: parsed?.code)
                VStack(alignment: .leading, spacing: 2) {
                    Text(conn.name).font(.system(size: 16, weight: .semibold)).foregroundStyle(K.text).lineLimit(1)
                    HStack(spacing: 6) {
                        Text(subtitle(parsed)).lineLimit(1).foregroundStyle(K.text2)
                        if pinging {
                            Text("· …").foregroundStyle(K.text2)
                        } else if let d = current?.delay {
                            Text("· \(Texts.ping(d))").foregroundStyle(K.ping(d))
                        }
                    }
                    .font(.system(size: 12.5))
                }
                Spacer(minLength: 4)
                if isActive && !list.isEmpty {
                    IconButton(symbol: "waveform.path.ecg", help: "Проверить задержку", spinning: pinging) { Task { await engine.testLatency() } }
                }
                if isSub {
                    IconButton(symbol: "arrow.clockwise", help: "Обновить подписку", spinning: refreshing) {
                        Task {
                            refreshing = true
                            await engine.refresh(conn.id)
                            refreshing = false
                        }
                    }
                }
                Menu {
                    if let url = conn.info?.supportUrl.flatMap(URL.init(string:)) { Button("Поддержка") { NSWorkspace.shared.open(url) } }
                    if let url = conn.info?.pageUrl.flatMap(URL.init(string:)) { Button("Страница подписки") { NSWorkspace.shared.open(url) } }
                    Divider()
                    Button(isSub ? "Удалить подписку…" : "Удалить конфигурацию…", role: .destructive) { confirmRemove = true }
                } label: {
                    Image(systemName: "ellipsis").font(.system(size: 15, weight: .semibold)).foregroundStyle(K.text).frame(width: 30, height: 30)
                }
                .menuStyle(.borderlessButton)
                .menuIndicator(.hidden)
                .fixedSize()
            }
            .padding(.horizontal, 6)
            .padding(.bottom, 12)

            if isSub { meta.padding(.horizontal, 6).padding(.bottom, 12) }

            Divider().overlay(K.line).padding(.horizontal, 4)

            VStack(spacing: 0) {
                if !isActive && !list.isEmpty { note("Выберите сервер — kl!ck перейдёт на это подключение.") }
                if list.isEmpty {
                    note(isActive || engine.servers[conn.id] != nil ? "Серверов пока нет. Обновите подписку — список придёт от панели." : "Загружаю список серверов…")
                }
                ForEach(list) { s in serverRow(s) }
            }
            .padding(.top, 6)
        }
        .padding(.horizontal, 10)
        .padding(.top, 14)
        .padding(.bottom, 8)
        .card()
        .confirmationDialog("Удалить «\(conn.name)»?", isPresented: $confirmRemove) {
            Button("Удалить", role: .destructive) { Task { await engine.remove(conn.id) } }
        }
    }

    private func subtitle(_ p: ParsedServer?) -> String {
        guard let current else { return conn.selectedServer ?? (isSub ? "Подписка" : "Одиночная конфигурация") }
        if let code = p?.code { return "\(Countries.name(code)) · \(p?.title.isEmpty == false ? p!.title : Texts.protocolName(current.kind))" }
        return current.name
    }

    @ViewBuilder private var meta: some View {
        let info = conn.info
        let used = info?.used ?? 0
        let total = info?.total ?? 0
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                if let exp = info?.expire, exp > 0 {
                    Text("Действует до \(Texts.date(exp))").foregroundStyle(K.text2)
                    Spacer()
                    let days = Texts.daysLeft(exp)
                    Text(days == 0 ? "истекает сегодня" : Texts.plural(days, ("день", "дня", "дней"))).fontWeight(.medium).foregroundStyle(days <= 3 ? K.orange : K.text)
                } else {
                    Text(total > 0 || used > 0 ? "Без срока" : "Срок и трафик панель не сообщает").foregroundStyle(K.text2)
                    Spacer()
                }
            }
            if total > 0 {
                let pct = min(1, used / total)
                HStack {
                    (Text("Осталось ").foregroundColor(K.text2) + Text(gb(max(0, total - used))).fontWeight(.medium).foregroundColor(K.text))
                    Spacer()
                    Text("из \(gb(total))").foregroundStyle(K.text2)
                }
                GeometryReader { g in
                    ZStack(alignment: .leading) {
                        Capsule().fill(K.fill)
                        Capsule().fill(pct > 0.9 ? K.orange : K.ok).frame(width: g.size.width * (1 - pct))
                    }
                }
                .frame(height: 8)
            } else if used > 0 {
                HStack {
                    (Text("Потрачено ").foregroundColor(K.text2) + Text(gb(used)).fontWeight(.medium).foregroundColor(K.text))
                    Spacer()
                    Text("без лимита").foregroundStyle(K.text2)
                }
            }
            if let a = info?.announce, !a.isEmpty { Text(a).font(.system(size: 12.5)).foregroundStyle(K.text3) }
        }
        .font(.system(size: 13))
    }

    private func note(_ text: String) -> some View {
        Text(text).font(.system(size: 12.5)).foregroundStyle(K.text2).frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 10).padding(.vertical, 8)
    }

    private func serverRow(_ s: Server) -> some View {
        let on = s.name == current?.name
        let p = Countries.parse(s.name, s.kind)
        return Button {
            if isActive { if !on { Task { await engine.selectServer(s.name) } } } else { onPickOther(s.name) }
        } label: {
            HStack(spacing: 10) {
                Radio(on: on)
                VStack(alignment: .leading, spacing: 1) {
                    HStack(spacing: 6) {
                        if let code = p.code, let f = FlagBadge.emoji(code) { Text(f).font(.system(size: 13)) }
                        Text(s.name).font(.system(size: 14, weight: .medium)).foregroundStyle(K.text).lineLimit(1)
                    }
                    Text(Texts.protocolName(s.kind) + (s.speed.map { " · \(Texts.mbps($0))" } ?? "")).font(.system(size: 12)).foregroundStyle(K.text2)
                }
                Spacer(minLength: 4)
                if pinging {
                    Skeleton()
                } else if s.delay != nil || engine.tested.contains(conn.id) {
                    Text(Texts.ping(s.delay)).font(.system(size: 13, weight: .medium)).monospacedDigit().foregroundStyle(K.ping(s.delay))
                }
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 9)
            .background(RoundedRectangle(cornerRadius: 16, style: .continuous).fill(on ? K.text.opacity(0.06) : .clear))
        }
        .buttonStyle(PressStyle(scale: 0.985))
    }
}

/// Значок-кнопка в шапке карточки; `spinning` — пока идёт действие.
struct IconButton: View {
    var symbol: String
    var help: String
    var spinning = false
    var action: () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol).font(.system(size: 15, weight: .medium))
                .foregroundStyle(spinning ? K.accent : K.text)
                .symbolEffect(.pulse, isActive: spinning)
                .frame(width: 30, height: 30)
                .background(RoundedRectangle(cornerRadius: 9, style: .continuous).fill(hover ? K.text.opacity(0.06) : .clear))
        }
        .buttonStyle(PressStyle(scale: 0.85))
        .disabled(spinning)
        .onHover { hover = $0 }
        .help(help)
    }
}

/// Заглушка «меряю» вместо задержки.
struct Skeleton: View {
    @State private var phase = false
    var body: some View {
        Capsule().fill(K.fill).frame(width: 42, height: 12)
            .opacity(phase ? 0.45 : 1)
            .onAppear { withAnimation(.easeInOut(duration: 0.7).repeatForever()) { phase = true } }
    }
}

struct Empty: View {
    var onAdd: () -> Void
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "power").font(.system(size: 34)).foregroundStyle(K.text2).frame(width: 84, height: 84).glass(Circle())
            Text("Нет подключений").font(.system(size: 20, weight: .semibold)).foregroundStyle(K.text)
            Text("Добавьте ссылку на подписку или одиночную конфигурацию — и можно включать.").font(.system(size: 13)).foregroundStyle(K.text2).multilineTextAlignment(.center)
            Button(action: onAdd) {
                Text("Добавить подключение").font(.system(size: 14, weight: .semibold)).foregroundStyle(.white).frame(maxWidth: .infinity).frame(height: 44)
            }
            .buttonStyle(PressStyle(scale: 0.98))
            .glass(RoundedRectangle(cornerRadius: 14, style: .continuous), tint: K.accent, interactive: true)
            .padding(.top, 8)
        }
        .padding(.top, 80)
    }
}

// MARK: Листы

struct SpeedSheet: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Скорость").font(.system(size: 20, weight: .bold)).foregroundStyle(K.text)
                Spacer()
                Button("Готово") { dismiss() }.keyboardShortcut(.defaultAction)
            }
            Text(engine.state?.server.map { "\($0) · сеанс" } ?? "VPN выключен").font(.system(size: 13)).foregroundStyle(K.text2)
            HStack(spacing: 8) {
                tile("Чтение", "arrow.down.circle", engine.traffic.down, engine.traffic.downSum)
                tile("Загрузка", "arrow.up.circle", engine.traffic.up, engine.traffic.upSum)
            }
            Graph(down: engine.traffic.downHist, up: engine.traffic.upHist)
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    if engine.speedTesting {
                        Text("Меряю скорость через сервер… до полуминуты").foregroundStyle(K.text2)
                    } else if let s = engine.speed {
                        Text("↓ \(s.down.map(Texts.mbps) ?? "—") · ↑ \(s.up.map(Texts.mbps) ?? "—")").font(.system(size: 14, weight: .semibold)).foregroundStyle(K.text)
                        if let srv = s.server { Text(srv).foregroundStyle(K.text2) }
                    } else {
                        Text(engine.state?.vpn == "connected" ? "Сколько реально даёт сервер" : "Замер — когда VPN включён").foregroundStyle(K.text2)
                    }
                }
                .font(.system(size: 13))
                Spacer()
                Button(engine.speed == nil ? "Проверить" : "Ещё раз") { Task { await engine.speedTest() } }
                    .disabled(engine.speedTesting || engine.state?.vpn != "connected")
            }
            .padding(14)
            .card(18)
        }
        .padding(20)
        .frame(width: 360)
        .background(K.bg)
    }

    private func tile(_ title: String, _ icon: String, _ now: Double, _ sum: Double) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Label(title, systemImage: icon).font(.system(size: 12.5)).foregroundStyle(K.text2)
            Text(rate(now)).font(.system(size: 22, weight: .semibold)).monospacedDigit().foregroundStyle(K.text)
            Text("всего \(gb(sum))").font(.system(size: 12)).foregroundStyle(K.text2)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .card(18)
    }
}

/// График минуты: чтение — акцентом с заливкой, загрузка — серой линией.
struct Graph: View {
    var down: [Double]
    var up: [Double]

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack {
                Text("Последние 60 секунд").foregroundStyle(K.text2)
                Spacer()
                Text("Пик: \(rate((down.max() ?? 0) * 1e6 / 8))").foregroundStyle(K.text2)
            }
            .font(.system(size: 12))
            GeometryReader { g in
                let maxV = max(50, (down + up).max() ?? 0) * 1.1
                ZStack {
                    path(down, maxV, g.size, closed: true).fill(K.ok.opacity(0.12))
                    path(down, maxV, g.size, closed: false).stroke(K.ok, lineWidth: 2)
                    path(up, maxV, g.size, closed: false).stroke(K.text2, lineWidth: 1.5)
                }
            }
            .frame(height: 90)
        }
        .padding(14)
        .card(18)
    }

    private func path(_ v: [Double], _ maxV: Double, _ size: CGSize, closed: Bool) -> Path {
        Path { p in
            guard v.count > 1 else { return }
            let dx = size.width / CGFloat(v.count - 1)
            for (i, x) in v.enumerated() {
                let pt = CGPoint(x: CGFloat(i) * dx, y: size.height - CGFloat(x / maxV) * size.height)
                if i == 0 { p.move(to: pt) } else { p.addLine(to: pt) }
            }
            if closed {
                p.addLine(to: CGPoint(x: size.width, y: size.height))
                p.addLine(to: CGPoint(x: 0, y: size.height))
                p.closeSubpath()
            }
        }
    }
}

/// «Добавить подключение»: ссылка из буфера — карточкой сверху, поле ссылки, файл конфигурации.
struct AddSheet: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.dismiss) private var dismiss
    @State private var link = ""
    @State private var name = ""
    @State private var busy = false
    @State private var clip: String?
    @State private var importing = false

    var body: some View {
        let d = Texts.detect(link)
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Добавить подключение").font(.system(size: 20, weight: .bold)).foregroundStyle(K.text)
                Spacer()
                Button("Отмена") { dismiss() }.keyboardShortcut(.cancelAction)
            }
            if let clip, clip != link.trimmingCharacters(in: .whitespacesAndNewlines) {
                let cd = Texts.detect(clip)
                HStack(spacing: 10) {
                    Circle().fill(cd.color).frame(width: 8, height: 8)
                    VStack(alignment: .leading, spacing: 1) {
                        Text("В буфере — \(cd.kind == "sub" ? "подписка" : "сервер")").font(.system(size: 13.5, weight: .semibold)).foregroundStyle(K.text)
                        Text(clip).font(.system(size: 12)).foregroundStyle(K.text2).lineLimit(1).truncationMode(.middle)
                    }
                    Spacer()
                    Button("Вставить") { link = clip; name = "" }
                }
                .padding(12)
                .background(RoundedRectangle(cornerRadius: 18, style: .continuous).fill(K.accent.opacity(0.1)))
            }
            TextEditor(text: $link)
                .font(.system(size: 13, design: .monospaced))
                .scrollContentBackground(.hidden)
                .padding(10)
                .frame(height: 96)
                .card(16)
            HStack(alignment: .top, spacing: 10) {
                Circle().fill(d.color).frame(width: 8, height: 8).padding(.top, 5)
                VStack(alignment: .leading, spacing: 2) {
                    Text(d.title).font(.system(size: 13, weight: .semibold)).foregroundStyle(K.text)
                    Text(d.text).font(.system(size: 12)).foregroundStyle(K.text2)
                }
            }
            if d.kind == "sub" || d.kind == "text" {
                TextField(d.kind == "text" ? "Название (иначе «\(d.name ?? "")»)" : "Название (необязательно)", text: $name).textFieldStyle(.roundedBorder)
            }
            HStack {
                Button("Из файла…") { importing = true }
                Spacer()
                Button(busy ? "Добавляю…" : "Добавить") { Task { await add(d) } }
                    .keyboardShortcut(.defaultAction)
                    .disabled(busy || !["sub", "link", "text"].contains(d.kind))
            }
        }
        .padding(20)
        .frame(width: 360)
        .background(K.bg)
        .onAppear(perform: readClipboard)
        .fileImporter(isPresented: $importing, allowedContentTypes: [.data, .text, .json, .yaml]) { result in
            guard case let .success(url) = result else { return }
            let access = url.startAccessingSecurityScopedResource()
            defer { if access { url.stopAccessingSecurityScopedResource() } }
            guard let text = try? String(contentsOf: url, encoding: .utf8) else { return }
            Task { if await engine.importFile(fileName: url.lastPathComponent, content: text) { dismiss() } }
        }
    }

    private func readClipboard() {
        let t = (NSPasteboard.general.string(forType: .string) ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let k = Texts.detect(t).kind
        clip = !t.isEmpty && (k == "sub" || k == "link") ? t : nil
    }

    private func add(_ d: Texts.Detect) async {
        busy = true
        defer { busy = false }
        let src = link.trimmingCharacters(in: .whitespacesAndNewlines)
        let ok = d.kind == "text"
            ? await engine.importFile(fileName: "\(name.isEmpty ? d.name ?? "Серверы" : name).txt", content: src)
            : await engine.add(source: src, name: d.kind == "sub" ? name : "")
        if ok { dismiss() }
    }
}
