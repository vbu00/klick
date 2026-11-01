import SwiftUI

/// Строка списка в карточке: слева текст (и необязательно плитка), справа — что угодно или стрелка.
struct Row<Trailing: View>: View {
    var title: String
    var sub: String?
    var tile: String?
    var action: (() -> Void)?
    @ViewBuilder var trailing: () -> Trailing
    @State private var hover = false

    var body: some View {
        let content = HStack(spacing: 12) {
            if let tile { Tile(label: tile) }
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 14, weight: .medium)).foregroundStyle(K.text)
                if let sub { Text(sub).font(.system(size: 12)).foregroundStyle(K.text2).fixedSize(horizontal: false, vertical: true) }
            }
            Spacer(minLength: 8)
            trailing()
            if action != nil { Image(systemName: "chevron.right").font(.system(size: 12, weight: .semibold)).foregroundStyle(K.text3) }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 12)
        .frame(minHeight: 56)
        .contentShape(Rectangle())

        if let action {
            Button(action: action) { content.background(hover ? K.text.opacity(0.04) : .clear) }
                .buttonStyle(.plain)
                .onHover { hover = $0 }
        } else {
            content
        }
    }
}

extension Row where Trailing == EmptyView {
    init(title: String, sub: String? = nil, tile: String? = nil, action: (() -> Void)? = nil) {
        self.init(title: title, sub: sub, tile: tile, action: action, trailing: { EmptyView() })
    }
}

/// Карточка списка: строки, разделённые линией.
struct CardList<Content: View>: View {
    @ViewBuilder var content: () -> Content
    var body: some View {
        VStack(spacing: 0) { _VariadicView.Tree(Divided()) { content() } }.card(22)
    }
}

private struct Divided: _VariadicView_MultiViewRoot {
    func body(children: _VariadicView.Children) -> some View {
        let last = children.last?.id
        ForEach(children) { child in
            child
            if child.id != last { Divider().overlay(K.line).padding(.leading, 16) }
        }
    }
}

/// Переключатель в цвет акцента.
struct Switch: View {
    var on: Bool
    var set: (Bool) -> Void
    var body: some View {
        Toggle("", isOn: Binding(get: { on }, set: set)).toggleStyle(.switch).tint(K.ok).labelsHidden().controlSize(.small)
    }
}

/// Плитка с буквой: цвет — по названию, как у веб-версии.
struct Tile: View {
    var label: String
    var size: CGFloat = 30
    var body: some View {
        let letter = label.trimmingCharacters(in: .whitespaces).first.map { String($0).uppercased() } ?? "?"
        let hue = Double(label.unicodeScalars.reduce(0) { ($0 &* 31 &+ Int($1.value)) & 0xffff } % 360) / 360
        RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
            .fill(Color(hue: hue, saturation: 0.45, brightness: 0.95))
            .frame(width: size, height: size)
            .overlay(Text(letter).font(.system(size: size * 0.45, weight: .semibold)).foregroundStyle(.black.opacity(0.7)))
    }
}

/// Пилюля «‹ Назад» в начале подэкрана.
struct BackPill: View {
    var label: String
    var action: () -> Void
    var body: some View {
        Button(action: action) {
            HStack(spacing: 4) {
                Image(systemName: "chevron.left").font(.system(size: 12, weight: .semibold))
                Text(label).font(.system(size: 13, weight: .medium))
            }
            .foregroundStyle(K.text)
            .padding(.horizontal, 12)
            .frame(height: 32)
        }
        .buttonStyle(PressStyle())
        .pill()
    }
}

struct ScreenTitle: View {
    var text: String
    var body: some View {
        Text(text).font(.system(size: 28, weight: .bold)).foregroundStyle(K.text).frame(maxWidth: .infinity, alignment: .leading).padding(.top, 6)
    }
}

/// Подпись раздела с необязательной ссылкой справа.
struct Caption: View {
    var text: String
    var link: String?
    var onLink: (() -> Void)?
    var body: some View {
        HStack {
            Text(text).caption()
            Spacer()
            if let link, let onLink {
                Button(link, action: onLink).buttonStyle(.plain).font(.system(size: 13, weight: .medium)).foregroundStyle(K.ok)
            }
        }
        .padding(.horizontal, 4)
        .padding(.top, 22)
        .padding(.bottom, 8)
    }
}

/// Экран со списком: прокрутка, поля, место под панель.
struct Screen<Content: View>: View {
    @ViewBuilder var content: () -> Content
    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) { content() }
                .padding(.horizontal, 16)
                .padding(.top, 34)
                .padding(.bottom, 110)
        }
        .scrollIndicators(.never)
    }
}
