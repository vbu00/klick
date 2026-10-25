import SwiftUI
import UIKit

/// Цвета веб-версии kl!ck (ui/src/lib/theme.ts): «Графит» в тёмной теме, «Светлая» в светлой. Размеры и
/// отступы ниже — из её стилей (ui/src/styles), чтобы бета выглядела так же. Переходный вид — до макета
/// родного интерфейса.
enum K {
    static let win = dyn(0x1a1a1d, 0xf5f5f7)
    static let card = dyn(0x222226, 0xffffff)
    static let cardHover = dyn(0x26262b, 0xf7f7f9)
    static let hover = dyn(0x2a2a2f, 0xefeff2)
    static let elem = dyn(0x2e2e33, 0xededf1)
    static let raised = dyn(0x3a3a40, 0xe3e3e8)
    static let ring = dyn(0x4a4a50, 0xc7c7cc)
    static let line = dyn(0x2c2c31, 0xe4e4e9)
    static let text = dyn(0xf2f2f4, 0x1c1c1e)
    static let text2 = dyn(0xc7c7cc, 0x3a3a3c)
    static let text3 = dyn(0xa1a1a8, 0x5f5f66)
    static let dim = dyn(0x8e8e93, 0x7c7c83)
    static let dim2 = dyn(0x6e6e73, 0xa0a0a7)
    static let dots = dyn(0x3c3c42, 0xd2d2d8)
    static let accent = Color(hex: 0x30d158)
    /// Текст на акценте: тёмный на ярком зелёном, белый в светлой теме (theme.ts: lum(accent)).
    static let onAccent = dyn(0x0b0b0c, 0xffffff)
    static let red = Color(hex: 0xff6961)
    static let orange = Color(hex: 0xffb340)

    private static func dyn(_ dark: UInt32, _ light: UInt32) -> Color {
        Color(UIColor { $0.userInterfaceStyle == .light ? UIColor(hex: light) : UIColor(hex: dark) })
    }

    /// Задержка: до 80 мс — зелёная, до 160 — обычная, дальше — оранжевая, нет ответа — красная (format.ts).
    static func ping(_ delay: Int?) -> Color {
        guard let d = delay else { return red }
        return d < 80 ? accent : d < 160 ? text : orange
    }
}

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: 1)
    }
}

extension Color {
    init(hex: UInt32) { self.init(UIColor(hex: hex)) }
}

// MARK: Значки

/// Значок веб-версии (ui/src/assets/icons, в каталоге Icons — scripts/ios/build-native.sh): цвет —
/// цвет текста, как `currentColor`.
struct KIcon: View {
    let name: String
    var size: CGFloat = 16

    init(_ name: String, size: CGFloat = 16) {
        self.name = name
        self.size = size
    }

    var body: some View {
        Image(name).renderingMode(.template).resizable().scaledToFit().frame(width: size, height: size)
    }
}

// MARK: Стекло

private struct GlassKey: EnvironmentKey { static let defaultValue = false }

extension EnvironmentValues {
    /// «Стекло» (Настройки → Тема), как в веб-версии.
    var klickGlass: Bool {
        get { self[GlassKey.self] }
        set { self[GlassKey.self] = newValue }
    }
}

/// Поверхность. Без стекла — сплошной цвет веб-версии. Со стеклом — системное стекло iPhone: Liquid Glass
/// на iOS 26 (Xcode 26), раньше — системный материал. Своих бликов и кромок нет: как у приложений Apple,
/// и «Понижение прозрачности» система учитывает сама. `interactive` — стекло откликается на касание.
struct KSurface<S: InsettableShape>: ViewModifier {
    @Environment(\.klickGlass) private var glass
    let shape: S
    let solid: Color
    var tint: Color?
    var interactive = false

    func body(content: Content) -> some View {
        if glass {
            #if compiler(>=6.2)
            if #available(iOS 26.0, *) {
                content.glassEffect(interactive ? .regular.tint(tint).interactive() : .regular.tint(tint), in: shape)
            } else {
                legacy(content)
            }
            #else
            legacy(content)
            #endif
        } else {
            content.background(solid, in: shape)
        }
    }

    @ViewBuilder
    private func legacy(_ content: Content) -> some View {
        if let tint {
            content.background(tint, in: shape)
        } else {
            content.background(.regularMaterial, in: shape)
        }
    }
}

extension View {
    func kSurface<S: InsettableShape>(_ shape: S, solid: Color = K.card, tint: Color? = nil, interactive: Bool = false) -> some View {
        modifier(KSurface(shape: shape, solid: solid, tint: tint, interactive: interactive))
    }

    /// Карточка `--card` со скруглением (20 — карточки и списки, 14 — карточки «Соединения»).
    func kCard(_ radius: CGFloat = 20) -> some View {
        kSurface(RoundedRectangle(cornerRadius: radius, style: .continuous))
    }
}

/// Фон окна. Со стеклом — спокойный перелив сверху вниз в цвете темы, без цветных пятен (glass.css): стеклу
/// есть что преломлять.
struct KBackground: View {
    @Environment(\.klickGlass) private var glass
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        Group {
            if glass {
                LinearGradient(colors: [Color(hex: scheme == .dark ? 0x313136 : 0xe6e9f0), K.win], startPoint: .top, endPoint: UnitPoint(x: 0.5, y: 0.6))
            } else {
                K.win
            }
        }
        .ignoresSafeArea()
    }
}

// MARK: Экран и разделы

/// Экран: `.content` веб-версии — поля 16, снизу место под панель вкладок; на iPad колонка до 560.
struct KScreen<Content: View>: View {
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) { content }
                .padding(.horizontal, 16)
                .padding(.bottom, 110)
                .frame(maxWidth: 560)
                .frame(maxWidth: .infinity)
        }
    }
}

/// `.screen-title`: 24, полужирный, сверху 14. `xl` — «Соединение»: 28, жирный, сверху 8.
struct KTitle: View {
    let text: String
    var xl = false

    init(_ text: String, xl: Bool = false) {
        self.text = text
        self.xl = xl
    }

    var body: some View {
        Text(text).font(.system(size: xl ? 28 : 24, weight: xl ? .bold : .semibold)).foregroundColor(K.text)
            .padding(.top, xl ? 8 : 14)
    }
}

/// `.caps`: 12, заглавными, разрядка 0.6.
struct KCaps: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text.uppercased()).font(.system(size: 12)).tracking(0.6).foregroundColor(K.dim)
    }
}

/// `.sec-head`: подпись раздела над списком, сверху 22; справа — необязательное действие.
struct KSectionHead<Trailing: View>: View {
    let title: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 8) {
            KCaps(title)
            Spacer()
            trailing
        }
        .frame(minHeight: 24)
        .padding(.top, 22)
    }
}

extension KSectionHead where Trailing == EmptyView {
    init(_ title: String) { self.init(title: title) { EmptyView() } }
}

/// `.card-list`: карточка списка строк, сверху 10.
struct KCardList<Content: View>: View {
    var top: CGFloat = 10
    @ViewBuilder var content: Content

    var body: some View {
        VStack(spacing: 0) { content }
            .frame(maxWidth: .infinity)
            .kCard()
            .padding(.top, top)
    }
}

/// `.row`: строка списка — 14 средний и подпись 12; справа — переключатель, значение или стрелка.
/// `wrap` — подпись в несколько строк. Строки разделяет линия с отступом 14 (`KDivider`).
struct KRow<Trailing: View>: View {
    let title: String
    var sub: String?
    var wrap = false
    var chevron = false
    var action: (() -> Void)?
    @ViewBuilder var trailing: Trailing

    var body: some View {
        if let action {
            Button(action: action) { content.contentShape(Rectangle()) }.buttonStyle(RowPress())
        } else {
            content
        }
    }

    private var content: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 14, weight: .medium)).foregroundColor(K.text)
                    .lineLimit(wrap ? nil : 1)
                if let sub, !sub.isEmpty {
                    Text(sub).font(.system(size: 12)).foregroundColor(K.dim)
                        .lineLimit(wrap ? nil : 1)
                        .lineSpacing(wrap ? 2 : 0)
                        .fixedSize(horizontal: false, vertical: wrap)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            trailing
            if chevron { KIcon("chevron", size: 14).foregroundColor(K.dim2) }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }
}

extension KRow where Trailing == EmptyView {
    init(_ title: String, sub: String? = nil, wrap: Bool = false, chevron: Bool = false, action: (() -> Void)? = nil) {
        self.init(title: title, sub: sub, wrap: wrap, chevron: chevron, action: action) { EmptyView() }
    }
}

/// Нажатая строка подсвечивается, как `:hover` у веб-версии.
private struct RowPress: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label.background(configuration.isPressed ? K.cardHover : Color.clear)
    }
}

/// Линия между строками карточки.
struct KDivider: View {
    var inset: CGFloat = 14
    var body: some View { Rectangle().fill(K.line).frame(height: 1).padding(.leading, inset) }
}

// MARK: Переключатель

/// `.toggle` веб-версии: 44×26, белый кружок. Со стеклом — системный переключатель iPhone (на iOS 26 —
/// с Liquid Glass) в цвете акцента.
struct KToggle: View {
    @Environment(\.klickGlass) private var glass
    @Binding var isOn: Bool

    var body: some View {
        if glass {
            Toggle("", isOn: $isOn).labelsHidden().toggleStyle(SwitchToggleStyle(tint: K.accent)).fixedSize()
        } else {
            Button {
                withAnimation(.easeInOut(duration: 0.2)) { isOn.toggle() }
            } label: {
                ZStack(alignment: isOn ? .trailing : .leading) {
                    Capsule().fill(isOn ? K.accent : K.raised).frame(width: 44, height: 26)
                    Circle().fill(Color.white).frame(width: 20, height: 20)
                        .shadow(color: .black.opacity(0.4), radius: 1.5, y: 1)
                        .padding(3)
                }
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(isOn ? .isSelected : [])
        }
    }
}

// MARK: Кнопки

/// `.btn-elem`: 28, скругление 14, фон `--elem`, 12 средний, значок через 6.
struct ElemButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(K.text)
            .padding(.horizontal, 10)
            .frame(height: 28)
            .kSurface(Capsule(), solid: K.elem, interactive: true)
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// `.btn-primary`: во всю ширину, 48, скругление 14, фон — цвет текста; выключенная — `--elem`.
struct PrimaryButtonStyle: ButtonStyle {
    var enabled = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 15, weight: .semibold))
            .foregroundColor(enabled ? K.win : K.dim)
            .frame(maxWidth: .infinity)
            .frame(height: 48)
            .kSurface(RoundedRectangle(cornerRadius: 14, style: .continuous), solid: enabled ? K.text : K.elem, tint: enabled ? K.text : nil, interactive: true)
            .scaleEffect(configuration.isPressed && enabled ? 0.98 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// `.btn-card` / `.cx-add`: во всю ширину, 44, скругление 12, фон карточки.
struct CardButtonStyle: ButtonStyle {
    var weight: Font.Weight = .medium

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 14, weight: weight))
            .foregroundColor(K.text)
            .frame(maxWidth: .infinity)
            .frame(height: 44)
            .kSurface(RoundedRectangle(cornerRadius: 12, style: .continuous), interactive: true)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.15), value: configuration.isPressed)
    }
}

/// `.pill-btn`: 30, скругление 15, фон карточки, 12 средний.
struct PillButtonStyle: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(K.text2)
            .padding(.horizontal, 12)
            .frame(height: 30)
            .kSurface(Capsule(), interactive: true)
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
    }
}

/// `.seg`: переключатель на 2–3 положения на подложке карточки.
struct KSeg<T: Hashable>: View {
    @Binding var value: T
    let options: [(T, String)]
    var height: CGFloat = 34
    var size: CGFloat = 13
    var weight: Font.Weight = .medium

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options.indices, id: \.self) { i in
                let (v, title) = options[i]
                Button {
                    withAnimation(.easeInOut(duration: 0.2)) { value = v }
                } label: {
                    Text(title)
                        .font(.system(size: size, weight: weight))
                        .foregroundColor(v == value ? K.text : K.dim)
                        .frame(maxWidth: .infinity)
                        .frame(height: height)
                        .background(v == value ? K.raised : Color.clear, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(3)
        .kSurface(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

/// `.radio`: кружок выбора 20 с точкой 8.
struct KRadio: View {
    let on: Bool
    var body: some View {
        Circle().strokeBorder(on ? K.accent : K.ring, lineWidth: 2).frame(width: 20, height: 20)
            .overlay(Circle().fill(on ? K.accent : Color.clear).frame(width: 8, height: 8))
    }
}

// MARK: Панель вкладок

/// `.nav`: плавающая капсула внизу — 4 кнопки 52×44, выбранная подсвечена `--raised`. Со стеклом —
/// Liquid Glass на iOS 26, раньше — системный материал.
struct KTabBar: View {
    @Environment(\.klickGlass) private var glass
    @Binding var tab: Int
    let items: [(icon: String, title: String)]

    var body: some View {
        HStack(spacing: 4) {
            ForEach(items.indices, id: \.self) { i in
                Button {
                    tab = i
                } label: {
                    KIcon(items[i].icon, size: 24)
                        .foregroundColor(tab == i ? K.text : K.dim)
                        .frame(width: 52, height: 44)
                        .background(tab == i ? K.raised : Color.clear, in: Capsule())
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(items[i].title)
                .accessibilityAddTraits(tab == i ? .isSelected : [])
            }
        }
        .padding(6)
        .modifier(BarSurface(glass: glass))
    }
}

private struct BarSurface: ViewModifier {
    let glass: Bool

    func body(content: Content) -> some View {
        if glass {
            content.kSurface(Capsule())
        } else {
            content
                .background(K.card.opacity(0.92), in: Capsule())
                .overlay(Capsule().stroke(K.line, lineWidth: 1))
                .shadow(color: .black.opacity(0.45), radius: 15, y: 10)
        }
    }
}

/// Кнопка питания: со стеклом на iOS 26 — Liquid Glass вокруг круга.
private struct GlassCircle: ViewModifier {
    @Environment(\.klickGlass) private var glass

    func body(content: Content) -> some View {
        #if compiler(>=6.2)
        if glass, #available(iOS 26.0, *) {
            content.glassEffect(.regular.interactive(), in: .circle)
        } else {
            content
        }
        #else
        content
        #endif
    }
}

extension View {
    func glassCircle() -> some View { modifier(GlassCircle()) }
}
