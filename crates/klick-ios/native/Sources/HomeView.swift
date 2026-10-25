import SwiftUI

/// Главная: кнопка VPN, подписка и её серверы, замер скорости. Подписку можно посмотреть, не
/// переключаясь на неё; активной она становится, когда в ней включают VPN или выбирают сервер.
struct HomeView: View {
    @EnvironmentObject var engine: Engine
    @State private var viewId: String?
    @State private var pendingSwitch: (id: String, server: String)?
    @State private var confirmSwitch = false

    private var connections: [Connection] { engine.settings?.connections ?? [] }
    private var current: Connection? { connections.first { $0.id == (viewId ?? engine.activeId) } ?? connections.first }
    private var isActive: Bool { current?.id == engine.activeId }

    var body: some View {
        NavigationView {
            List {
                Section {
                    PowerBlock(viewing: current?.id)
                }
                .listRowBackground(Color.clear)

                if let conn = current {
                    connectionSection(conn)
                    serversSection(conn)
                    if isActive && engine.state?.vpn == "connected" { speedSection }
                } else {
                    Section {
                        VStack(spacing: 10) {
                            Image(systemName: "link.badge.plus").font(.system(size: 36)).foregroundColor(.klickAccent)
                            Text("Добавьте подписку").font(.headline)
                            Text("Вкладка «Добавить»: ссылка от вашего VPN-сервиса или QR-код.")
                                .font(.footnote).foregroundStyle(.secondary).multilineTextAlignment(.center)
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 12)
                    }
                }
            }
            .listStyle(.insetGrouped)
            .navigationTitle("kl!ck")
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
        .navigationViewStyle(.stack)
    }

    // MARK: Подписка

    @ViewBuilder
    private func connectionSection(_ conn: Connection) -> some View {
        Section {
            if connections.count > 1 {
                Picker("Подписка", selection: Binding(get: { conn.id }, set: { id in
                    viewId = id
                    Task { await engine.viewServers(id) }
                })) {
                    ForEach(connections) { c in Text(c.name).tag(c.id) }
                }
            }
            VStack(alignment: .leading, spacing: 6) {
                HStack {
                    Text(conn.name).font(.headline)
                    Spacer()
                    if isActive && engine.state?.vpn == "connected" {
                        Text("Работает").font(.caption.weight(.semibold)).foregroundColor(.klickAccent)
                            .padding(.horizontal, 8).padding(.vertical, 3)
                            .background(Color.klickAccent.opacity(0.15), in: Capsule())
                    }
                }
                if let info = conn.info { trafficLines(info) }
            }
            .padding(.vertical, 4)
            if let info = conn.info {
                if let announce = info.announce, !announce.isEmpty {
                    Text(announce).font(.footnote).foregroundStyle(.secondary)
                }
                if let s = info.supportUrl, let url = URL(string: s) { Link(destination: url) { Label("Поддержка", systemImage: "questionmark.bubble") } }
                if let s = info.pageUrl, let url = URL(string: s) { Link(destination: url) { Label("Страница подписки", systemImage: "safari") } }
            }
        } header: {
            Text("Подключение")
        }
    }

    @ViewBuilder
    private func trafficLines(_ info: SubInfo) -> some View {
        let total = info.total ?? 0
        if total > 0 {
            ProgressView(value: min(info.used, total), total: total)
                .tint(info.used / total > 0.9 ? .orange : .klickAccent)
            Text("Трафик \(Texts.gb(info.used)) из \(Texts.gb(total))").font(.footnote).foregroundStyle(.secondary)
        } else if info.used > 0 {
            Text("Трафик \(Texts.gb(info.used)) · без лимита").font(.footnote).foregroundStyle(.secondary)
        }
        if let expire = info.expire {
            Text("Действует до \(Texts.date(expire)) · \(Texts.daysLeft(expire)) дн.").font(.footnote).foregroundStyle(.secondary)
        } else if total > 0 || info.used > 0 {
            Text("Без срока").font(.footnote).foregroundStyle(.secondary)
        }
    }

    // MARK: Серверы

    @ViewBuilder
    private func serversSection(_ conn: Connection) -> some View {
        let list = engine.servers[conn.id] ?? []
        let selected = list.first(where: \.selected)?.name ?? conn.selectedServer ?? list.first?.name
        Section {
            if list.isEmpty {
                Text(isActive ? "Серверов пока нет — потяните вниз, чтобы обновить подписку" : "Загружаю список серверов…")
                    .font(.footnote).foregroundStyle(.secondary)
                    .task { await engine.viewServers(conn.id) }
            }
            ForEach(list) { server in
                Button {
                    pick(server, in: conn, selected: selected)
                } label: {
                    ServerRow(server: server, current: server.name == selected, pinging: isActive && engine.pinging)
                }
                .foregroundColor(.primary)
            }
        } header: {
            HStack {
                Text("Серверы · \(list.count)")
                Spacer()
                if isActive && list.count > 1 {
                    Button(engine.pinging ? "Проверяю…" : "Задержка") { Task { await engine.testLatency() } }
                        .font(.caption.weight(.semibold))
                        .disabled(engine.pinging)
                }
            }
            .textCase(nil)
        } footer: {
            if !isActive && !list.isEmpty { Text("Выберите сервер — kl!ck перейдёт на это подключение.") }
        }
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

    // MARK: Скорость

    private var speedSection: some View {
        Section {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    if engine.speedTesting {
                        Text("Меряю скорость через сервер…").foregroundStyle(.secondary)
                    } else if let s = engine.speed {
                        Text("↓ \(s.down.map(Texts.mbps) ?? "—") · ↑ \(s.up.map(Texts.mbps) ?? "—")").font(.headline).monospacedDigit()
                        if let server = s.server { Text(server).font(.caption).foregroundStyle(.secondary) }
                    } else {
                        Text("Сколько реально даёт сервер").foregroundStyle(.secondary)
                    }
                }
                Spacer()
                if engine.speedTesting {
                    ProgressView()
                } else {
                    Button(engine.speed == nil ? "Проверить" : "Ещё раз") { Task { await engine.speedTest() } }
                        .buttonStyle(.bordered)
                }
            }
        } header: {
            Text("Скорость")
        }
    }
}

/// Кнопка питания, состояние и время подключения.
struct PowerBlock: View {
    @EnvironmentObject var engine: Engine
    let viewing: String?

    private var busy: Bool { engine.powerPending || ["connecting", "reconnecting"].contains(engine.state?.vpn ?? "") }

    var body: some View {
        VStack(spacing: 14) {
            Text(Texts.status(engine.state?.vpn))
                .font(.subheadline)
                .foregroundColor(engine.state?.vpn == "server_down" || engine.state?.vpn == "error" ? .orange : .secondary)
            if engine.state?.vpn == "connected", let since = engine.state?.since {
                Text(Date(timeIntervalSince1970: since), style: .timer)
                    .font(.system(size: 34, weight: .semibold, design: .rounded))
                    .monospacedDigit()
            } else {
                Text("0:00:00")
                    .font(.system(size: 34, weight: .semibold, design: .rounded))
                    .foregroundStyle(.tertiary)
            }
            Button {
                Task { await engine.power(viewing: viewing) }
            } label: {
                ZStack {
                    Circle().fill(engine.isRunning ? Color.klickAccent : Color(.secondarySystemGroupedBackground))
                    if busy {
                        ProgressView().scaleEffect(1.6).tint(engine.isRunning ? .white : .klickAccent)
                    } else {
                        Image(systemName: "power").font(.system(size: 50, weight: .semibold))
                            .foregroundColor(engine.isRunning ? .white : .klickAccent)
                    }
                }
                .frame(width: 140, height: 140)
                .shadow(color: engine.isRunning ? Color.klickAccent.opacity(0.45) : .clear, radius: 18)
            }
            .buttonStyle(.plain)
            .glassCircle()
            .accessibilityLabel(engine.isRunning ? "Выключить VPN" : "Включить VPN")
            if engine.state?.vpn == "connected" {
                HStack(spacing: 16) {
                    Label(Texts.rate(engine.traffic.down), systemImage: "arrow.down")
                    Label(Texts.rate(engine.traffic.up), systemImage: "arrow.up")
                }
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)
    }
}

struct ServerRow: View {
    let server: Server
    let current: Bool
    let pinging: Bool

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: current ? "checkmark.circle.fill" : "circle")
                .foregroundColor(current ? .klickAccent : Color(.tertiaryLabel))
                .font(.title3)
            VStack(alignment: .leading, spacing: 2) {
                Text(server.name).lineLimit(1)
                Text(Texts.protocolName(server.kind)).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
            if let speed = server.speed {
                Text(Texts.mbps(speed)).font(.caption).foregroundStyle(.secondary)
            }
            Text(pinging ? "…" : server.delay.map { "\($0) мс" } ?? "—")
                .font(.callout.monospacedDigit())
                .foregroundColor(delayColor)
        }
    }

    private var delayColor: Color {
        guard !pinging, let d = server.delay else { return .secondary }
        return d < 150 ? .klickAccent : d < 400 ? .orange : .red
    }
}
