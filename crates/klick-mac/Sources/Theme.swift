import AppKit
import SwiftUI

/// Палитры прототипа Димы: светлая и «Графит». Цвет выбирается по оформлению окна (светлое / тёмное).
private struct Palette {
    var bg, surface, line, fill, text, text2, text3: NSColor
}

private func rgb(_ v: UInt32, _ a: CGFloat = 1) -> NSColor {
    NSColor(srgbRed: CGFloat((v >> 16) & 0xff) / 255, green: CGFloat((v >> 8) & 0xff) / 255, blue: CGFloat(v & 0xff) / 255, alpha: a)
}

private let light = Palette(bg: rgb(0xf5f5f7), surface: .white, line: rgb(0xe4e4e4), fill: rgb(0xe9e9ec), text: rgb(0x202020), text2: rgb(0x767676), text3: rgb(0xa3a3a8))
private let dark = Palette(bg: rgb(0x1a1a1d), surface: rgb(0x232327), line: rgb(0x3a3a40), fill: rgb(0x2e2e33), text: rgb(0xf2f2f4), text2: rgb(0x8e8e93), text3: rgb(0x6e6e73))

private func isDark(_ a: NSAppearance) -> Bool { a.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua }

private func dyn(_ pick: @escaping (Palette) -> NSColor) -> Color {
    Color(nsColor: NSColor(name: nil) { pick(isDark($0) ? dark : light) })
}

private func dyn(light l: UInt32, dark d: UInt32) -> Color {
    Color(nsColor: NSColor(name: nil) { rgb(isDark($0) ? d : l) })
}

/// Цвета окна. Имена `dim`, `accent`, `red` — те же, что у iOS β: на них опираются общие Texts.
enum K {
    static let bg = dyn(\.bg)
    static let surface = dyn(\.surface)
    static let line = dyn(\.line)
    static let fill = dyn(\.fill)
    static let text = dyn(\.text)
    static let text2 = dyn(\.text2)
    static let text3 = dyn(\.text3)
    static let dim = text2
    static let ok = dyn(light: 0x1fa64a, dark: 0x32d74b)
    static let orange = dyn(light: 0xd97a00, dark: 0xff9f0a)
    static let red = dyn(light: 0xe5372b, dark: 0xff453a)
    /// Акцент своей темы (`#34c759` по умолчанию); меняется из настроек.
    static var accentHex: UInt32 = 0x34c759
    static var accent: Color { Color(nsColor: rgb(accentHex)) }

    static func ping(_ delay: Int?) -> Color {
        guard let delay else { return red }
        return delay < 80 ? ok : delay < 160 ? text : orange
    }

    static func parseHex(_ s: String) -> UInt32? {
        UInt32(s.trimmingCharacters(in: CharacterSet(charactersIn: "#")), radix: 16)
    }
}

extension View {
    /// Liquid Glass (macOS 26): настоящее стекло системы — преломление, блики, реакция на нажатие.
    /// На macOS 14–15 — системный материал с размытием и тонкой линией.
    @ViewBuilder
    func glass<S: Shape>(_ shape: S, tint: Color? = nil, interactive: Bool = false) -> some View {
        if #available(macOS 26.0, *) {
            self.glassEffect(Self.makeGlass(tint: tint, interactive: interactive), in: shape)
        } else {
            self
                .background(shape.fill(tint.map { AnyShapeStyle($0.opacity(0.85)) } ?? AnyShapeStyle(.regularMaterial)))
                .overlay(shape.stroke(K.line, lineWidth: 0.5))
        }
    }

    @available(macOS 26.0, *)
    private static func makeGlass(tint: Color?, interactive: Bool) -> Glass {
        var g = Glass.regular
        if let tint { g = g.tint(tint) }
        if interactive { g = g.interactive() }
        return g
    }

    /// Карточка содержимого — сплошная, как в прототипе: стекло по правилам Apple — для управления, не для текста.
    func card(_ radius: CGFloat = 26) -> some View {
        background(RoundedRectangle(cornerRadius: radius, style: .continuous).fill(K.surface))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).stroke(K.line, lineWidth: 0.5))
    }

    /// Сплошная капсула (чипы): стекло — только на слое управления (панель, верхний ряд, кнопка питания),
    /// чтобы его было в меру.
    func pill(selected: Bool = false) -> some View {
        background(Capsule().fill(K.surface))
            .overlay(Capsule().stroke(selected ? K.text.opacity(0.25) : K.line, lineWidth: selected ? 1 : 0.5))
    }

    /// Подпись раздела: заглавными, мелко, серым.
    func caption() -> some View {
        font(.system(size: 12, weight: .regular)).tracking(0.4).textCase(.uppercase).foregroundStyle(K.text2)
    }
}

/// Нажатие уменьшает, как `:active { transform: scale(0.96) }` у прототипа.
struct PressStyle: ButtonStyle {
    var scale: CGFloat = 0.96
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .contentShape(Rectangle())
            .scaleEffect(configuration.isPressed ? scale : 1)
            .animation(.spring(response: 0.25, dampingFraction: 0.7), value: configuration.isPressed)
    }
}

/// Кружок выбора: кольцо и точка акцента.
struct Radio: View {
    var on: Bool
    var body: some View {
        ZStack {
            Circle().stroke(on ? K.accent : K.text3, lineWidth: 1.5)
            Circle().fill(K.accent).frame(width: 10, height: 10).scaleEffect(on ? 1 : 0.01)
        }
        .frame(width: 20, height: 20)
        .animation(.spring(response: 0.35, dampingFraction: 0.6), value: on)
    }
}

/// Флаг страны эмодзи в кружке (на Mac эмодзи флагов рисуются, в отличие от Windows).
struct FlagBadge: View {
    var code: String?
    var size: CGFloat = 42
    var body: some View {
        ZStack {
            Circle().fill(K.fill)
            if let code, let flag = Self.emoji(code) {
                Text(flag).font(.system(size: size * 0.78))
            } else {
                Image(systemName: "globe").font(.system(size: size * 0.45)).foregroundStyle(K.text2)
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
    }

    static func emoji(_ code: String) -> String? {
        let up = code.uppercased()
        guard up.count == 2, up.unicodeScalars.allSatisfy({ $0.value >= 65 && $0.value <= 90 }) else { return nil }
        return String(String.UnicodeScalarView(up.unicodeScalars.compactMap { UnicodeScalar(127_397 + $0.value) }))
    }
}
