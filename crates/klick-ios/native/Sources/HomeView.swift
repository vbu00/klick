import SwiftUI

/// Главная: кнопка VPN, подписка и её серверы, замер скорости — как в веб-версии kl!ck. Подписку можно
/// посмотреть, не переключаясь на неё; активной она становится, когда в ней включают VPN или выбирают сервер.
struct HomeView: View {
    @EnvironmentObject var engine: Engine
    @State private var viewId: String?
    @State private var pendingSwitch: (id: String, server: String)?
    @State private var confirmSwitch = false

    private var connections: [Connection] { engine.settings?.connections ?? [] }
    private var current: Connection? { connections.first { $0.id == (viewId ?? engine.activeId) } ?? connections.first }
    private var isActive: Bool { current?.id == engine.activeId }
    private var connected: Bool { engine.state?.vpn == "connected" }

    var body: some View {
        KScreen {
            PowerBlock(viewing: current?.id)

            if let conn = current {
                if connections.count > 1 { chips }
                SubscriptionCard(conn: conn, active: isActive, servers: engine.servers[conn.id] ?? [])
                    .padding(.top, connections.count > 1 ? 10 : 22)
                if isActive && connected {
                    speedCard.padding(.top, 12)
                    trafficTiles.padding(.top, 12)
                }
                serversSection(conn)
            } else {
                emptyCard.padding(.top, 22)
            }
        }
        .refreshable {
            if let conn = current, conn.kind == "subscription" { await engine.refresh(conn.id) } else { await engine.reload() }
        }
        .confirmationDialog("Сменить подключение?", isPresented: $confirmSwitch, titleVisibility: .visible) {
            Button("Переключить") {
                guard let p = pendingSwitch else { return }
                Task { if await engine.selectConnection(p.id) { await engine.selectServer(p.server) } }
            }
            Button("Отмена", role: .cancel) {}
        } message: {
            Text("Текущее соединение разорвётся на пару секунд, и трафик пойдёт через выбранный сервер.")
        }
    }

    // MARK: Подписки

    private var chips: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                ForEach(connections) { c in
                    KChip(title: c.name, on: c.id == current?.id, live: c.id == engine.activeId && connected) {
                        viewId = c.id
                        Task { await engine.viewServers(c.id) }
                    }
                }
            }
            .padding(.horizontal, 1)
            .padding(.vertical, 2)
        }
        .padding(.top, 22)
    }

    private var emptyCard: some View {
        VStack(spacing: 10) {
            Image(systemName: "link.badge.plus").font(.system(size: 30)).foregroundColor(K.accent)
            Text("Добавьте подписку").font(.system(size: 17, weight: .semibold)).foregroundColor(K.text)
            Text("Вкладка «Добавить»: ссылка от вашего VPN-сервиса или QR-код.")
                .font(.system(size: 13)).foregroundColor(K.dim).multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 26)
        .padding(.horizontal, 16)
        .kCard()
    }

    // MARK: Скорость и трафик

    private var speedCard: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                if engine.speedTesting {
                    Text("Меряю скорость через сервер…").font(.system(size: 13)).foregroundColor(K.dim)
                } else if let s = engine.speed {
                    Text("↓ \(s.down.map(Texts.mbps) ?? "—")  ↑ \(s.up.map(Texts.mbps) ?? "—")")
                        .font(.system(size: 15, weight: .semibold)).monospacedDigit().foregroundColor(K.text)
                    if let server = s.server { Text(server).font(.system(size: 12)).foregroundColor(K.dim).lineLimit(1) }
                } else {
                    Text("Сколько реально даёт сервер").font(.system(size: 13)).foregroundColor(K.dim)
                }
            }
            Spacer(minLength: 8)
            if engine.speedTesting {
                ProgressView().tint(K.dim)
            } else {
                Button {
                    Task { await engine.speedTest() }
                } label: {
                    Label(engine.speed == nil ? "Проверить скорость" : "Ещё раз", systemImage: "arrow.clockwise")
                }
                .buttonStyle(PillButtonStyle())
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .kCard(radius: 18)
    }

    private var trafficTiles: some View {
        HStack(spacing: 12) {
            tile("Чтение", "icloud.and.arrow.down", engine.traffic.down)
            tile("Загрузка", "icloud.and.arrow.up", engine.traffic.up)
        }
    }

    private func tile(_ title: String, _ icon: String, _ rate: Double) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(title).font(.system(size: 13)).foregroundColor(K.dim)
                Spacer()
                Image(systemName: icon).font(.system(size: 15)).foregroundColor(K.dim)
            }
            Text(Texts.rate(rate)).font(.system(size: 22, weight: .bold)).monospacedDigit().foregroundColor(K.text)
                .lineLimit(1).minimumScaleFactor(0.7)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .kCard(radius: 18)
    }

    // MARK: Серверы

    @ViewBuilder
    private func serversSection(_ conn: Connection) -> some View {
        let list = engine.servers[conn.id] ?? []
        let selected = list.first(where: \.selected)?.name ?? conn.selectedServer ?? list.first?.name
        KSection(title: "Серверы · \(list.count)") {
            if isActive && list.count > 1 {
                Button(engine.pinging ? "Проверяю…" : "Задержка") { Task { await engine.testLatency() } }
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundColor(K.accent)
                    .disabled(engine.pinging)
            }
        }
        VStack(spacing: 0) {
            if list.isEmpty {
                Text(isActive ? "Серверов пока нет — потяните вниз, чтобы обновить подписку" : "Загружаю список серверов…")
                    .font(.system(size: 13)).foregroundColor(K.dim)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
                    .task { await engine.viewServers(conn.id) }
            }
            ForEach(Array(list.enumerated()), id: \.element.id) { i, server in
                if i > 0 { KDivider(inset: 52) }
                Button {
                    pick(server, in: conn, selected: selected)
                } label: {
                    ServerRow(server: server, current: server.name == selected, pinging: isActive && engine.pinging)
                }
                .buttonStyle(.plain)
            }
        }
        .kCard()
        if !isActive && !list.isEmpty { KFootnote("Выберите сервер — kl!ck перейдёт на это подключение.") }
    }

    private func pick(_ server: Server, in conn: Connection, selected: String?) {
        if isActive {
            if server.name != selected { Task { await engine.selectServer(server.name) } }
        } else if engine.isRunning {
            pendingSwitch = (conn.id, server.name)
            confirmSwitch = true
        } else {
            Task { if await engine.selectConnection(conn.id) { await engine.selectServer(server.name) } }
        }
    }
}

// MARK: Карточка подписки

/// Карточка подписки веб-версии: значок, имя и сервер, трафик, срок, «Обновить», ссылки панели.
struct SubscriptionCard: View {
    @EnvironmentObject var engine: Engine
    @Environment(\.openURL) private var openURL
    let conn: Connection
    let active: Bool
    let servers: [Server]

    private var working: Bool { active && engine.state?.vpn == "connected" }
    /// Тот же сервер, что отмечен в списке: выбранный, сохранённый в подключении или первый.
    private var server: Server? { servers.first(where: \.selected) ?? servers.first { $0.name == conn.selectedServer } ?? servers.first }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
            if let info = conn.info, conn.kind == "subscription" {
                VStack(alignment: .leading, spacing: 12) {
                    traffic(info)
                    HStack(spacing: 8) {
                        expiry(info)
                        Spacer(minLength: 8)
                        Button {
                            Task { await engine.refresh(conn.id) }
                        } label: {
                            Label("Обновить", systemImage: "arrow.clockwise")
                        }
                        .buttonStyle(PillButtonStyle())
                    }
                    if let announce = info.announce, !announce.isEmpty {
                        Text(announce).font(.system(size: 12)).foregroundColor(K.dim).fixedSize(horizontal: false, vertical: true)
                    }
                    links(info)
                }
                .padding(.horizontal, 16)
                .padding(.bottom, 16)
            }
        }
        .kCard()
    }

    private var head: some View {
        HStack(spacing: 14) {
            Image(systemName: conn.kind == "subscription" ? "globe" : "link")
                .font(.system(size: 20))
                .foregroundColor(K.dim)
                .frame(width: 52, height: 52)
                .background(K.elem.opacity(0.8), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 8) {
                    Text(conn.name).font(.system(size: 17, weight: .semibold)).foregroundColor(K.text).lineLimit(1)
                    if working {
                        Text("Работает").font(.system(size: 11, weight: .semibold)).foregroundColor(K.accent)
                            .padding(.horizontal, 7).padding(.vertical, 2)
                            .background(K.accent.opacity(0.16), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
                    }
                }
                HStack(spacing: 6) {
                    Text(server?.name ?? (conn.kind == "subscription" ? "подписка" : "одиночный сервер"))
                        .lineLimit(1)
                        .foregroundColor(K.dim)
                    if let delay = server?.delay {
                        Text("·").foregroundColor(K.dim2)
                        Text("\(delay) мс").foregroundColor(delay < 150 ? K.accent : delay < 400 ? K.orange : K.red)
                    }
                }
                .font(.system(size: 13))
            }
            Spacer(minLength: 0)
            Menu {
                if conn.kind == "subscription" {
                    Button { Task { await engine.refresh(conn.id) } } label: { Label("Обновить подписку", systemImage: "arrow.clockwise") }
                }
                Button(role: .destructive) { Task { await engine.remove(conn.id) } } label: { Label("Удалить", systemImage: "trash") }
            } label: {
                Image(systemName: "ellipsis").rotationEffect(.degrees(90))
                    .font(.system(size: 16, weight: .semibold)).foregroundColor(K.dim)
                    .frame(width: 36, height: 44)
            }
        }
        .padding(16)
    }

    @ViewBuilder
    private func traffic(_ info: SubInfo) -> some View {
        let total = info.total ?? 0
        if total > 0 || info.used > 0 {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("Трафик").foregroundColor(K.dim)
                    Spacer()
                    Text(total > 0 ? "\(Texts.gb(info.used)) из \(Texts.gb(total))" : "\(Texts.gb(info.used)) · без лимита")
                        .fontWeight(.semibold).foregroundColor(K.text)
                }
                .font(.system(size: 12))
                if total > 0 {
                    GeometryReader { g in
                        ZStack(alignment: .leading) {
                            Capsule().fill(K.elem)
                            Capsule().fill(info.used / total > 0.9 ? K.orange : K.text)
                                .frame(width: max(4, g.size.width * min(1, info.used / total)))
                        }
                    }
                    .frame(height: 4)
                }
            }
        }
    }

    @ViewBuilder
    private func expiry(_ info: SubInfo) -> some View {
        if let expire = info.expire {
            (Text("Действует до ") + Text(Texts.date(expire)).foregroundColor(K.text).fontWeight(.semibold) + Text(" · \(Texts.daysLeft(expire)) дн."))
                .font(.system(size: 12)).foregroundColor(K.dim)
        } else {
            Text("Без срока").font(.system(size: 12)).foregroundColor(K.dim)
        }
    }

    @ViewBuilder
    private func links(_ info: SubInfo) -> some View {
        let support = info.supportUrl.flatMap(URL.init(string:))
        let page = info.pageUrl.flatMap(URL.init(string:))
        if support != nil || page != nil {
            HStack(spacing: 8) {
                if let support { Button("Поддержка") { openURL(support) }.buttonStyle(PillButtonStyle()) }
                if let page { Button("Страница подписки") { openURL(page) }.buttonStyle(PillButtonStyle()) }
            }
        }
    }
}

// MARK: Кнопка питания

/// Состояние, время подключения, кнопка питания и режим — как в веб-версии.
struct PowerBlock: View {
    @EnvironmentObject var engine: Engine
    let viewing: String?

    private var vpn: String { engine.state?.vpn ?? "off" }
    private var busy: Bool { engine.powerPending || ["connecting", "reconnecting"].contains(vpn) }

    var body: some View {
        VStack(spacing: 0) {
            Text(Texts.status(engine.state?.vpn))
                .font(.system(size: 15))
                .foregroundColor(vpn == "server_down" ? K.orange : vpn == "error" ? K.red : K.dim)
            Group {
                if vpn == "connected", let since = engine.state?.since {
                    Text(Date(timeIntervalSince1970: since), style: .timer)
                } else {
                    Text("0:00:00")
                }
            }
            .font(.system(size: 34, weight: .bold))
            .monospacedDigit()
            .foregroundColor(K.text)
            .padding(.top, 4)

            PowerButton(state: vpn, busy: busy, running: engine.isRunning) {
                Task { await engine.power(viewing: viewing) }
            }
            .padding(.top, 14)

            modeChip.padding(.top, 14)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, 28)
    }

    private var modeChip: some View {
        HStack(spacing: 8) {
            Circle().fill(vpn == "connected" ? K.accent : K.dim2).frame(width: 6, height: 6)
            Text(engine.settings?.routing == "all_vpn" ? "Весь трафик через VPN" : "VPN для выбранного")
                .font(.system(size: 13, weight: .medium))
                .foregroundColor(K.text2)
        }
        .padding(.horizontal, 16)
        .frame(height: 34)
        .kSurface(Capsule())
    }
}

/// Круглая кнопка 84 pt в поле 120 pt: выключено — карточка, работает — акцент с пульсом и точками вокруг,
/// подключается — дуга по кругу, сервер не отвечает — оранжевая, ошибка — красная.
struct PowerButton: View {
    let state: String
    let busy: Bool
    let running: Bool
    let action: () -> Void

    private var live: Bool { state == "connected" }
    private var tone: Color {
        if state == "server_down" { return K.orange }
        if state == "error" { return K.red }
        return K.accent
    }

    var body: some View {
        Button(action: action) {
            ZStack {
                if live {
                    DotsHalo().transition(.opacity)
                    PulseRing()
                }
                Circle()
                    .fill(live ? K.accent : K.card)
                    .overlay(Circle().stroke(live ? K.accent : busy || state == "server_down" || state == "error" ? tone.opacity(busy ? 0.45 : 1) : K.line, lineWidth: 1))
                    .frame(width: 84, height: 84)
                    .glassCircle()
                if busy { SpinnerArc(color: tone).frame(width: 84, height: 84) }
                Image(systemName: "power")
                    .font(.system(size: 30, weight: .medium))
                    .foregroundColor(live ? K.onAccent : busy || running || state == "error" ? tone : K.dim)
            }
            .frame(width: 240, height: 150)
            .contentShape(Circle().inset(by: 60))
        }
        .buttonStyle(PowerPressStyle())
        .animation(.easeInOut(duration: 0.25), value: live)
        .accessibilityLabel(running ? "Выключить VPN" : "Включить VPN")
    }
}

private struct PowerPressStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.scaleEffect(configuration.isPressed ? 0.96 : 1).animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

/// Пульс: кружок цвета акцента растёт до 120 pt и гаснет.
private struct PulseRing: View {
    @State private var on = false
    var body: some View {
        Circle().fill(K.accent.opacity(0.35))
            .frame(width: 84, height: 84)
            .scaleEffect(on ? 1.43 : 1)
            .opacity(on ? 0 : 1)
            .onAppear { withAnimation(.easeOut(duration: 2.2).repeatForever(autoreverses: false)) { on = true } }
    }
}

/// Дуга, бегущая по кругу, пока VPN подключается.
private struct SpinnerArc: View {
    let color: Color
    @State private var turn = false
    var body: some View {
        Circle().trim(from: 0, to: 0.22)
            .stroke(color, style: StrokeStyle(lineWidth: 2, lineCap: .round))
            .rotationEffect(.degrees(turn ? 360 : 0))
            .onAppear { withAnimation(.linear(duration: 0.9).repeatForever(autoreverses: false)) { turn = true } }
    }
}

/// Точечное поле вокруг включённой кнопки: гуще у кнопки, к краю гаснет.
private struct DotsHalo: View {
    var body: some View {
        Canvas { ctx, size in
            let c = CGPoint(x: size.width / 2, y: size.height / 2)
            let step: CGFloat = 11
            let rx = size.width / 2, ry = size.height / 2
            var y = step / 2
            while y < size.height {
                var x = step / 2
                while x < size.width {
                    let dx = (x - c.x) / rx, dy = (y - c.y) / ry
                    let d = (dx * dx + dy * dy).squareRoot()
                    let r = hypot(x - c.x, y - c.y)
                    if d < 1, r > 46 {
                        ctx.fill(Path(ellipseIn: CGRect(x: x - 1.1, y: y - 1.1, width: 2.2, height: 2.2)),
                                 with: .color(K.accent.opacity(0.45 * Double(1 - d))))
                    }
                    x += step
                }
                y += step
            }
        }
        .allowsHitTesting(false)
    }
}

// MARK: Строка сервера

struct ServerRow: View {
    let server: Server
    let current: Bool
    let pinging: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: current ? "checkmark.circle.fill" : "circle")
                .foregroundColor(current ? K.accent : K.dim2)
                .font(.system(size: 22))
                .frame(width: 24)
            VStack(alignment: .leading, spacing: 2) {
                Text(server.name).font(.system(size: 15, weight: current ? .semibold : .regular)).foregroundColor(K.text).lineLimit(1)
                Text(Texts.protocolName(server.kind)).font(.system(size: 12)).foregroundColor(K.dim)
            }
            Spacer(minLength: 8)
            if let speed = server.speed {
                Text(Texts.mbps(speed)).font(.system(size: 12)).foregroundColor(K.dim)
            }
            Text(pinging ? "…" : server.delay.map { "\($0) мс" } ?? "—")
                .font(.system(size: 14, weight: .medium)).monospacedDigit()
                .foregroundColor(delayColor)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 11)
        .contentShape(Rectangle())
    }

    private var delayColor: Color {
        guard !pinging, let d = server.delay else { return K.dim }
        return d < 150 ? K.accent : d < 400 ? K.orange : K.red
    }
}
