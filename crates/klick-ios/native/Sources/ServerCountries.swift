import SwiftUI
import UIKit

/// `.flag`: флаг страны в круге (рисунки — ui/src/lib/flags.ts, в каталоге Icons как `flag-XX`); флага
/// нет — кружок с кодом, страны нет — глобус.
struct FlagView: View {
    let code: String?
    var size: CGFloat = 28

    var body: some View {
        ZStack {
            if let code, UIImage(named: "flag-\(code)") != nil {
                Image("flag-\(code)").resizable().scaledToFill()
            } else {
                K.elem
                if let code {
                    Text(code).font(.system(size: size * 0.36, weight: .bold)).tracking(0.3).foregroundColor(K.dim)
                } else {
                    KIcon("globe", size: size * 0.54).foregroundColor(K.dim)
                }
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        // Тонкий обод: белые полосы флага не сливаются со светлой карточкой.
        .overlay(Circle().strokeBorder(K.text.opacity(0.14), lineWidth: 1))
    }
}

/// Название с флагом-эмодзи в начале: флаг — кружком, как в веб-версии, дальше — остальное имя.
struct FlagName: View {
    let name: String
    var size: CGFloat

    var body: some View {
        let split = Countries.splitFlag(name)
        if let code = split.code, UIImage(named: "flag-\(code)") != nil {
            HStack(spacing: size * 0.35) {
                FlagView(code: code, size: size * 1.05)
                Text(split.rest).lineLimit(1)
            }
        } else {
            Text(name).lineLimit(1)
        }
    }
}

/// Серверы по странам — как ui/src/components/ServerCountries.tsx: строка страны (флаг, название, сколько
/// серверов, протоколы, лучшая задержка), по нажатию — её серверы. Порядок стран от задержек не зависит.
struct ServerCountries: View {
    let list: [Server]
    let current: Server?
    let pinging: Bool
    let tested: Bool
    let onPick: (String) -> Void

    /// Раскрытые страны живут, пока открыто приложение.
    private static var remembered: Set<String>?
    @State private var open: Set<String> = []
    /// Страна выбранного сервера — сверху, но только на момент открытия: выбор в другой стране строки не двигает.
    @State private var pin: String??

    private var cur: ParsedServer? { current.map { Countries.parse($0.name, $0.kind) } }
    private var curKey: String? { cur.map { $0.code ?? "" } }

    var body: some View {
        VStack(spacing: 2) {
            ForEach(Countries.group(list, pin: pin ?? curKey), id: \.key) { g in
                group(g)
            }
        }
        .onAppear {
            if pin == nil { pin = .some(curKey) }
            if let r = Self.remembered { open = r } else if let k = curKey { open = [k] }
        }
    }

    private func group(_ g: CountryGroup) -> some View {
        let isOpen = open.contains(g.key)
        let isCur = g.key == curKey
        let best = g.items.compactMap(\.server.delay).min()
        return VStack(spacing: 0) {
            Button {
                withAnimation(.easeOut(duration: 0.28)) {
                    if open.contains(g.key) { open.remove(g.key) } else { open.insert(g.key) }
                }
                Self.remembered = open
            } label: {
                HStack(spacing: 12) {
                    FlagView(code: g.code)
                        .overlay(Circle().strokeBorder(isCur ? K.accent : Color.clear, lineWidth: 1.5).padding(-3.5))
                    VStack(alignment: .leading, spacing: 3) {
                        HStack(alignment: .firstTextBaseline, spacing: 6) {
                            Text(g.name).font(.system(size: 14, weight: .semibold)).foregroundColor(K.text).lineLimit(1)
                            Text("\(g.items.count)").font(.system(size: 12)).monospacedDigit().foregroundColor(K.dim)
                        }
                        HStack(spacing: 4) {
                            if isCur, let current, let cur {
                                Circle().fill(K.accent).frame(width: 6, height: 6).padding(.trailing, 2)
                                Text(title(current, cur)).font(.system(size: 12)).foregroundColor(K.text2).lineLimit(1)
                            } else {
                                ForEach(g.tags, id: \.self) { Tag(text: $0) }
                            }
                        }
                        .frame(height: 16)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    Ping(delay: best, pinging: pinging, tested: tested, size: 13)
                    KIcon("chevron", size: 14).foregroundColor(K.dim2).rotationEffect(.degrees(isOpen ? 90 : 0))
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 9)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            if isOpen {
                VStack(spacing: 2) {
                    ForEach(g.items, id: \.server.name) { item in
                        row(item.server, item.p)
                    }
                }
                .padding(.bottom, 6)
                .padding(.leading, 9)
                .overlay(alignment: .leading) { Rectangle().fill(K.line).frame(width: 1.5) }
                .padding(.leading, 21)
                .transition(.opacity)
            }
        }
    }

    private func row(_ s: Server, _ p: ParsedServer) -> some View {
        let on = s.name == current?.name
        let mute = tested && !pinging && s.delay == nil
        let proto = !p.title.isEmpty && !p.named ? Texts.protocolName(s.kind) : nil
        return Button {
            if !on { onPick(s.name) }
        } label: {
            HStack(spacing: 10) {
                Circle().strokeBorder(on ? K.accent : K.ring, lineWidth: 2).frame(width: 18, height: 18)
                    .overlay(Circle().fill(on ? K.accent : Color.clear).frame(width: 8, height: 8))
                Text(title(s, p)).font(.system(size: 13.5, weight: on ? .semibold : .medium))
                    .foregroundColor(mute ? K.dim : K.text).lineLimit(1)
                    .frame(maxWidth: .infinity, alignment: .leading)
                if let proto { Tag(text: proto) }
                if let speed = s.speed { Text(Texts.mbps(speed)).font(.system(size: 11)).monospacedDigit().foregroundColor(K.dim) }
                Ping(delay: s.delay, pinging: pinging, tested: tested, size: 12.5)
            }
            .padding(.horizontal, 8)
            .frame(height: 38)
            .background(on ? K.hover : Color.clear, in: RoundedRectangle(cornerRadius: K.r(10), style: .continuous))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func title(_ s: Server, _ p: ParsedServer) -> String { p.title.isEmpty ? Texts.protocolName(s.kind) : p.title }
}

/// `.cg-tag`: метка протокола — 16, скругление 5, 10 полужирный.
private struct Tag: View {
    let text: String
    var body: some View {
        Text(text).font(.system(size: 10, weight: .semibold)).tracking(0.3).foregroundColor(K.text3)
            .padding(.horizontal, 5).frame(height: 16)
            .background(K.elem, in: RoundedRectangle(cornerRadius: 5, style: .continuous))
    }
}

/// Задержка: «…» во время замера, пусто до первого замера — nil тогда не значит «нет ответа».
private struct Ping: View {
    let delay: Int?
    let pinging: Bool
    let tested: Bool
    let size: CGFloat

    var body: some View {
        if pinging {
            Text("…").font(.system(size: size)).foregroundColor(K.dim).frame(minWidth: 44, alignment: .trailing)
        } else if let delay {
            Text("\(delay) мс").font(.system(size: size)).monospacedDigit().foregroundColor(K.ping(delay)).frame(minWidth: 44, alignment: .trailing)
        } else if tested {
            Text("нет ответа").font(.system(size: 11, weight: .semibold)).foregroundColor(K.red.opacity(0.9))
                .padding(.horizontal, 8).frame(height: 20)
                .background(K.red.opacity(0.13), in: Capsule())
        } else {
            Color.clear.frame(width: 44, height: 1)
        }
    }
}
