import SwiftUI
import UIKit

/// Цвета веб-версии kl!ck (ui/src/lib/theme.ts): «Графит» в тёмной теме, «Светлая» в светлой. Переходный
/// вид — до макета родного интерфейса.
enum K {
    static let win = dyn(0x1a1a1d, 0xf5f5f7)
    static let card = dyn(0x222226, 0xffffff)
    static let elem = dyn(0x2e2e33, 0xededf1)
    static let raised = dyn(0x3a3a40, 0xe3e3e8)
    static let line = dyn(0x2c2c31, 0xe4e4e9)
    static let sunken = dyn(0x141416, 0xfafafb)
    static let text = dyn(0xf2f2f4, 0x1c1c1e)
    static let text2 = dyn(0xc7c7cc, 0x3a3a3c)
    static let dim = dyn(0x8e8e93, 0x7c7c83)
    static let dim2 = dyn(0x6e6e73, 0xa0a0a7)
    static let accent = Color(hex: 0x30d158)
    static let onAccent = dyn(0x0b0b0c, 0xffffff)
    static let red = Color(hex: 0xff6961)
    static let orange = Color(hex: 0xffb340)

    private static func dyn(_ dark: UInt32, _ light: UInt32) -> Color {
        Color(UIColor { $0.userInterfaceStyle == .light ? UIColor(hex: light) : UIColor(hex: dark) })
    }
}

extension UIColor {
    convenience init(hex: UInt32) {
        self.init(red: CGFloat(hex >> 16 & 0xff) / 255, green: CGFloat(hex >> 8 & 0xff) / 255, blue: CGFloat(hex & 0xff) / 255, alpha: 1)
    }
}

extension Color {
    init(hex: UInt32) { self.init(UIColor(hex: hex)) }
    static let klickAccent = K.accent
}

// MARK: Экран, разделы, карточки

/// Экран: прокрутка на фоне окна, колонка до 560 pt (iPad), снизу — место под панель вкладок.
struct KScreen<Content: View>: View {
    var title: String?
    @ViewBuilder var content: Content

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let title {
                    Text(title).font(.system(size: 30, weight: .bold)).foregroundColor(K.text)
                        .padding(.top, 12).padding(.leading, 4).padding(.bottom, 6)
                }
                content
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 120)
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
    }
}

/// Фон окна. Со стеклом — спокойный перелив сверху вниз в цвете темы, без цветных пятен: стеклу есть что
/// преломлять.
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

/// Подпись раздела: «МАРШРУТИЗАЦИЯ», справа — необязательное действие.
struct KSection<Trailing: View>: View {
    let title: String
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack {
            Text(title.uppercased()).font(.system(size: 12, weight: .semibold)).tracking(0.6).foregroundColor(K.dim)
            Spacer()
            trailing
        }
        .padding(.horizontal, 4)
        .padding(.top, 22)
        .padding(.bottom, 8)
    }
}

extension KSection where Trailing == EmptyView {
    init(_ title: String) { self.init(title: title) { EmptyView() } }
}

/// Пояснение под карточкой.
struct KFootnote: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text).font(.system(size: 12)).foregroundColor(K.dim).padding(.horizontal, 4).padding(.top, 8)
            .fixedSize(horizontal: false, vertical: true)
    }
}

// MARK: Стекло

private struct GlassKey: EnvironmentKey { static let defaultValue = false }

extension EnvironmentValues {
    /// «Стекло» (Оформление → Стекло), как в веб-версии.
    var klickGlass: Bool {
        get { self[GlassKey.self] }
        set { self[GlassKey.self] = newValue }
    }
}

/// Поверхность. Без стекла — сплошной цвет веб-версии. Со стеклом — системное стекло iPhone: Liquid Glass
/// на iOS 26 (Xcode 26), раньше — системный материал. Своих бликов и кромок нет: как у приложений Apple,
/// и «Понижение прозрачности» система учитывает сама.
struct KSurface<S: InsettableShape>: ViewModifier {
    @Environment(\.klickGlass) private var glass
    let shape: S
    let solid: Color

    func body(content: Content) -> some View {
        if glass {
            #if compiler(>=6.2)
            if #available(iOS 26.0, *) {
                content.glassEffect(.regular, in: shape)
            } else {
                content.background(.regularMaterial, in: shape)
            }
            #else
            content.background(.regularMaterial, in: shape)
            #endif
        } else {
            content.background(solid, in: shape)
        }
    }
}

extension View {
    func kSurface<S: InsettableShape>(_ shape: S, solid: Color = K.card) -> some View {
        modifier(KSurface(shape: shape, solid: solid))
    }

    /// Карточка веб-версии: `--card`, скругление 20; со стеклом — стеклянная.
    func kCard(radius: CGFloat = 20) -> some View {
        kSurface(RoundedRectangle(cornerRadius: radius, style: .continuous))
    }
}

/// Строка карточки: название, подпись и что-то справа (переключатель, меню, значение).
struct KRow<Trailing: View>: View {
    let title: String
    var subtitle: String?
    @ViewBuilder var trailing: Trailing

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 15, weight: .semibold)).foregroundColor(K.text)
                if let subtitle, !subtitle.isEmpty {
                    Text(subtitle).font(.system(size: 12)).foregroundColor(K.dim).fixedSize(horizontal: false, vertical: true)
                }
            }
            Spacer(minLength: 8)
            trailing
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 13)
        .frame(minHeight: 56)
    }
}

extension KRow where Trailing == EmptyView {
    init(_ title: String, _ subtitle: String? = nil) { self.init(title: title, subtitle: subtitle) { EmptyView() } }
}

/// Разделитель строк внутри карточки.
struct KDivider: View {
    var inset: CGFloat = 16
    var body: some View { Rectangle().fill(K.line).frame(height: 1).padding(.leading, inset) }
}

/// Переключатель цвета акцента.
struct KToggle: View {
    @Binding var isOn: Bool
    var body: some View {
        Toggle("", isOn: $isOn).labelsHidden().toggleStyle(SwitchToggleStyle(tint: K.accent))
    }
}

/// Значение справа с меню выбора: «Тёмная ⌃⌄».
struct KMenuValue: View {
    let text: String
    var body: some View {
        HStack(spacing: 5) {
            Text(text).font(.system(size: 14)).foregroundColor(K.dim).lineLimit(1)
            Image(systemName: "chevron.up.chevron.down").font(.system(size: 11, weight: .semibold)).foregroundColor(K.dim2)
        }
    }
}

// MARK: Кнопки

/// Заливка кнопки или чипа. Без стекла — цвет веб-версии. Со стеклом на iOS 26 — Liquid Glass, который
/// откликается на касание (`tint` — цветное стекло акцентной кнопки); раньше — системный материал, а
/// акцентная остаётся сплошной.
struct KFill<S: InsettableShape>: ViewModifier {
    @Environment(\.klickGlass) private var glass
    let shape: S
    let solid: Color
    var tint: Color?

    func body(content: Content) -> some View {
        if glass {
            #if compiler(>=6.2)
            if #available(iOS 26.0, *) {
                content.glassEffect(.regular.tint(tint).interactive(), in: shape)
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
            content.background(.thinMaterial, in: shape)
        }
    }
}

extension View {
    func kFill<S: InsettableShape>(_ shape: S, solid: Color, tint: Color? = nil) -> some View {
        modifier(KFill(shape: shape, solid: solid, tint: tint))
    }
}

/// Текст на акцентной кнопке: на цветном стекле iOS 26 — белый, как у системных кнопок, иначе — тёмный
/// на зелёном, как в веб-версии.
private func onAccent(_ glass: Bool) -> Color {
    #if compiler(>=6.2)
    if glass, #available(iOS 26.0, *) { return .white }
    #endif
    return K.onAccent
}

/// Кнопка-пилюля: «Обновить», «Проверить скорость», «Поддержка».
struct PillButtonStyle: ButtonStyle {
    @Environment(\.klickGlass) private var glass
    var accent = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundColor(accent ? onAccent(glass) : K.text)
            .padding(.horizontal, 12)
            .frame(height: 32)
            .kFill(Capsule(), solid: accent ? K.accent : K.elem, tint: accent ? K.accent : nil)
            .scaleEffect(configuration.isPressed ? 0.96 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

/// Большая кнопка во всю ширину: «Добавить».
struct WideButtonStyle: ButtonStyle {
    @Environment(\.klickGlass) private var glass
    var enabled = true

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 16, weight: .semibold))
            .foregroundColor(enabled ? onAccent(glass) : K.dim)
            .frame(maxWidth: .infinity)
            .frame(height: 50)
            .kFill(RoundedRectangle(cornerRadius: 16, style: .continuous), solid: enabled ? K.accent : K.card, tint: enabled ? K.accent : nil)
            .scaleEffect(configuration.isPressed ? 0.98 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}

/// Чип выбора подписки над карточкой.
struct KChip: View {
    @Environment(\.klickGlass) private var glass
    let title: String
    let on: Bool
    var live = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if live { Circle().fill(K.accent).frame(width: 6, height: 6) }
                Text(title).lineLimit(1)
            }
            .font(.system(size: 12, weight: .medium))
            .foregroundColor(on ? K.text : K.dim)
            .padding(.horizontal, 12)
            .frame(height: 30)
            .modifier(ChipFill(on: on, glass: glass))
        }
        .buttonStyle(.plain)
    }
}

/// Чип: со стеклом — стекло (выбранный — с оттенком акцента), без — обводка, выбранный залит.
private struct ChipFill: ViewModifier {
    let on: Bool
    let glass: Bool

    func body(content: Content) -> some View {
        if glass {
            content.kFill(Capsule(), solid: .clear, tint: on ? K.accent.opacity(0.35) : nil)
        } else {
            content
                .background(on ? K.raised : Color.clear, in: Capsule())
                .overlay(Capsule().stroke(on ? K.raised : K.line, lineWidth: 1))
        }
    }
}

// MARK: Liquid Glass (iOS 26)

/// Кнопка питания: со стеклом на iOS 26 (Xcode 26) — Liquid Glass; иначе — как есть.
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

/// Панель вкладок: со стеклом — Liquid Glass на iOS 26, раньше — размытие; без стекла — карточка с
/// обводкой и тенью, как в веб-версии.
private struct BarSurface: ViewModifier {
    @Environment(\.klickGlass) private var glass

    func body(content: Content) -> some View {
        #if compiler(>=6.2)
        if glass, #available(iOS 26.0, *) {
            content.glassEffect(.regular, in: .capsule)
        } else {
            solid(content)
        }
        #else
        solid(content)
        #endif
    }

    @ViewBuilder
    private func solid(_ content: Content) -> some View {
        if glass {
            content.background(.regularMaterial, in: Capsule()).shadow(color: .black.opacity(0.18), radius: 12, y: 6)
        } else {
            content
                .background(K.card.opacity(0.94), in: Capsule())
                .overlay(Capsule().stroke(K.line, lineWidth: 1))
                .shadow(color: .black.opacity(0.35), radius: 15, y: 10)
        }
    }
}

extension View {
    func glassCircle() -> some View { modifier(GlassCircle()) }
    func glassCapsule() -> some View { modifier(BarSurface()) }
}

// MARK: Панель вкладок

/// Плавающая панель вкладок веб-версии: капсула внизу, выбранная — подсвечена.
struct KTabBar: View {
    @Binding var tab: Int
    let items: [(icon: String, title: String)]

    var body: some View {
        HStack(spacing: 4) {
            ForEach(items.indices, id: \.self) { i in
                Button {
                    tab = i
                } label: {
                    Image(systemName: items[i].icon)
                        .font(.system(size: 18, weight: .semibold))
                        .foregroundColor(tab == i ? K.text : K.dim)
                        .frame(width: 58, height: 44)
                        .background(tab == i ? K.raised : Color.clear, in: Capsule())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(items[i].title)
                .accessibilityAddTraits(tab == i ? .isSelected : [])
            }
        }
        .padding(6)
        .glassCapsule()
    }
}
