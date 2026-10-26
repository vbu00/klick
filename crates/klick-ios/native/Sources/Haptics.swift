import SwiftUI
import UIKit

/// Тактильная отдача, как у системных приложений: «тук» на нажатие, щелчок на выбор, успех и ошибка —
/// на итог. Генераторы живут всё время: так отклик без задержки.
@MainActor
enum Haptics {
    private static let light = UIImpactFeedbackGenerator(style: .light)
    private static let medium = UIImpactFeedbackGenerator(style: .medium)
    private static let selection = UISelectionFeedbackGenerator()
    private static let notification = UINotificationFeedbackGenerator()

    /// Кнопка питания, «Добавить» и подобное.
    static func tap() { medium.impactOccurred() }
    /// Тумблер, удаление строки.
    static func soft() { light.impactOccurred() }
    /// Выбор: сервер, сегмент, подписка, тема.
    static func pick() { selection.selectionChanged() }
    static func success() { notification.notificationOccurred(.success) }
    static func warning() { notification.notificationOccurred(.warning) }
    static func error() { notification.notificationOccurred(.error) }
}

extension View {
    /// Цифры меняются прокруткой, как в системных приложениях (iOS 16+); раньше — сменой текста.
    @ViewBuilder
    func kNumeric<V: Equatable>(_ value: V) -> some View {
        if #available(iOS 16.0, *) {
            self.contentTransition(.numericText()).animation(.spring(response: 0.3, dampingFraction: 0.9), value: value)
        } else {
            self
        }
    }
}

extension Animation {
    /// Пружина интерфейса: раскрытие карточки, смена состояния кнопки.
    static let kSpring = Animation.spring(response: 0.38, dampingFraction: 0.82)
}

/// Смахнуть строку влево — «Удалить», как в списках iOS; смахнуть до конца — сразу удалить.
struct SwipeToDelete: ViewModifier {
    let action: () -> Void
    @State private var offset: CGFloat = 0
    @State private var width: CGFloat = 0
    private let button: CGFloat = 82

    func body(content: Content) -> some View {
        ZStack(alignment: .trailing) {
            if offset < 0 {
                Button {
                    remove()
                } label: {
                    Text("Удалить").font(.system(size: 14, weight: .semibold)).foregroundColor(.white)
                        .frame(width: max(button, -offset)).frame(maxHeight: .infinity)
                }
                .background(K.red)
                .buttonStyle(.plain)
            }
            content
                .background(K.card)
                .offset(x: offset)
                // Вместе с прокруткой, а не вместо: вертикальное движение строка пропускает.
                .simultaneousGesture(
                    DragGesture(minimumDistance: 18)
                        .onChanged { g in
                            guard abs(g.translation.width) > abs(g.translation.height) else { return }
                            offset = min(0, g.translation.width + (offset <= -button ? -button : 0))
                        }
                        .onEnded { g in
                            if -g.translation.width > width * 0.6 {
                                remove()
                            } else {
                                withAnimation(.kSpring) { offset = -g.translation.width > button / 2 ? -button : 0 }
                            }
                        }
                )
                .onTapGesture { if offset < 0 { withAnimation(.kSpring) { offset = 0 } } }
        }
        .clipped()
        .background(GeometryReader { g in Color.clear.onAppear { width = g.size.width } })
    }

    private func remove() {
        Haptics.soft()
        withAnimation(.kSpring) { offset = -width }
        action()
    }
}
