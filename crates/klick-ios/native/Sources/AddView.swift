import SwiftUI
import UIKit
import UniformTypeIdentifiers

/// «Добавить подключение» — как ui/src/screens/Add.tsx: по ссылке (или QR-коду) и из файла.
struct AddView: View {
    @EnvironmentObject var engine: Engine
    @Binding var tab: Int
    @State private var mode = "link"
    @State private var link = ""
    @State private var name = ""
    @State private var busy = false
    @State private var picking = false

    private var d: Texts.Detect { Texts.detect(link) }
    private var canAdd: Bool { ["sub", "link", "text"].contains(d.kind) && !busy }

    var body: some View {
        KScreen {
            KTitle("Добавить подключение")
            KSeg(value: $mode, options: [("link", "По ссылке"), ("file", "Из файла")]).padding(.top, 18)
            if mode == "link" { byLink } else { fromFile }
        }
        .fileImporter(isPresented: $picking, allowedContentTypes: [.yaml, .json, .plainText, .data]) { result in
            guard case let .success(url) = result else { return }
            Task { await addFile(url) }
        }
    }

    // MARK: По ссылке

    @ViewBuilder
    private var byLink: some View {
        Button {
            scan()
        } label: {
            HStack(spacing: 8) {
                KIcon("qr", size: 18)
                Text("Сканировать QR-код")
            }
        }
        .buttonStyle(CardButtonStyle())
        .disabled(busy)
        .padding(.top, 12)

        ZStack(alignment: .topLeading) {
            if link.isEmpty {
                Text("https://panel.example.com/sub/a1b2… или vless://…")
                    .font(.system(size: 13, design: .monospaced)).foregroundColor(K.dim2)
                    .padding(.horizontal, 14).padding(.vertical, 14)
                    .allowsHitTesting(false)
            }
            LinkEditor(text: $link)
                .padding(.horizontal, 9).padding(.vertical, 6)
        }
        .frame(height: 110)
        .background(K.card, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous)
            .strokeBorder(d.kind == "bad" ? K.red.opacity(0.55) : d.kind == "empty" ? K.line : K.accent.opacity(0.55), lineWidth: 1))
        .padding(.top, 16)
        .accessibilityLabel("Ссылка на подписку или конфигурацию")

        HStack(spacing: 10) {
            Circle().fill(d.color).frame(width: 8, height: 8)
            VStack(alignment: .leading, spacing: 2) {
                Text(d.title).font(.system(size: 13, weight: .semibold)).foregroundColor(K.text)
                Text(d.text).font(.system(size: 12)).foregroundColor(K.text3).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .kCard(12)
        .padding(.top, 12)

        if d.kind == "sub" || d.kind == "text" {
            TextField("", text: $name, prompt: Text(d.kind == "text" ? "Название (иначе «\(d.name ?? "")»)" : "Название (необязательно)").foregroundColor(K.dim2))
                .font(.system(size: 14))
                .foregroundColor(K.text)
                .padding(.horizontal, 14)
                .frame(height: 44)
                .kCard(12)
                .padding(.top, 10)
        }

        Button {
            Task { await addLink() }
        } label: {
            Text(busy ? "Добавляем…" : "Добавить")
        }
        .buttonStyle(PrimaryButtonStyle(enabled: canAdd))
        .disabled(!canAdd)
        .padding(.top, 14)

        KCaps("Что можно вставить").padding(.top, 26)
        VStack(spacing: 0) {
            help("globe", "Ссылка на подписку",
                 "https://… от Remnawave, Marzban и подобных — в форматах Clash, sing-box, Xray или списком ссылок; ключ Outline ssconf://. Даёт список серверов, лимит трафика и срок действия. Обновляется автоматически.")
            Rectangle().fill(K.line).frame(height: 1)
            help("rules", "Одиночная конфигурация", "vless://, vmess://, trojan://, ss://, hysteria2:// — прямое соединение с одним сервером.")
        }
        .kCard()
        .padding(.top, 10)
    }

    private func help(_ icon: String, _ title: String, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 12) {
            KIcon(icon, size: 18).foregroundColor(K.dim)
                .frame(width: 32, height: 32)
                .background(K.elem, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 13, weight: .semibold)).foregroundColor(K.text)
                Text(text).font(.system(size: 12)).foregroundColor(K.text3).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }

    // MARK: Из файла

    @ViewBuilder
    private var fromFile: some View {
        Button {
            picking = true
        } label: {
            VStack(spacing: 10) {
                KIcon("upload", size: 34)
                Text(busy ? "Импортируем…" : "Выберите файл").font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                Text("из «Файлов» или iCloud Drive").font(.system(size: 12))
            }
            .foregroundColor(K.dim)
            .frame(maxWidth: .infinity)
            .frame(height: 190)
            .background(K.card.opacity(0.5), in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: 16, style: .continuous).strokeBorder(K.raised, style: StrokeStyle(lineWidth: 1.5, dash: [6, 4])))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(busy)
        .padding(.top, 16)

        (Text("Поддерживаются конфиги Mihomo / Clash (") + Text(".yaml").bold() + Text(", ") + Text(".json").bold()
            + Text("), sing-box и Xray (") + Text(".json").bold() + Text("), WireGuard и AmneziaWG (") + Text(".conf").bold()
            + Text("), Shadowsocks и Outline (") + Text(".json").bold() + Text("), списки ссылок (") + Text(".txt").bold()
            + Text("). Из файла берутся серверы; файл копируется в приложение — оригинал можно удалить."))
            .font(.system(size: 12)).foregroundColor(K.text3).lineSpacing(2)
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .kCard(14)
            .padding(.top, 12)
    }

    // MARK: Действия

    private func addLink() async {
        guard canAdd else { return }
        busy = true
        let text = link.trimmingCharacters(in: .whitespacesAndNewlines)
        let title = name.trimmingCharacters(in: .whitespaces)
        let detected = d
        let ok = detected.kind == "text"
            ? await engine.importText(name: title.isEmpty ? detected.name ?? "Серверы" : title, content: text)
            : await engine.add(source: text, name: detected.kind == "sub" ? title : "")
        busy = false
        if ok {
            link = ""
            name = ""
            tab = Tab.home.rawValue
        }
    }

    private func addFile(_ url: URL) async {
        busy = true
        defer { busy = false }
        let scoped = url.startAccessingSecurityScopedResource()
        defer { if scoped { url.stopAccessingSecurityScopedResource() } }
        guard let content = try? String(contentsOf: url, encoding: .utf8) else {
            engine.toast = Toast("Не удалось прочитать файл", url.lastPathComponent, tone: "bad")
            return
        }
        if await engine.importFile(fileName: url.lastPathComponent, content: content) { tab = Tab.home.rawValue }
    }

    /// QR-код подписки — камерой или из фото (Sources/klick-ios/QRScanner.swift): ссылка встаёт в поле,
    /// добавляет её человек сам. Сканер ждёт ответа — не на главном потоке.
    private func scan() {
        Task.detached(priority: .userInitiated) {
            let raw = klickScanQr()
            let text = raw.map { String(cString: $0) }
            if let raw { free(raw) }
            await MainActor.run {
                if let text = text?.trimmingCharacters(in: .whitespacesAndNewlines), !text.isEmpty {
                    link = Texts.fromDeepLink(text)
                    name = ""
                }
            }
        }
    }
}

/// Поле ссылки в несколько строк (`textarea`): моноширинное 13, без автозамен и заглавных.
private struct LinkEditor: UIViewRepresentable {
    @Binding var text: String

    func makeUIView(context: Context) -> UITextView {
        let v = UITextView()
        v.backgroundColor = .clear
        v.font = .monospacedSystemFont(ofSize: 13, weight: .regular)
        v.textColor = UIColor(K.text)
        v.autocapitalizationType = .none
        v.autocorrectionType = .no
        v.spellCheckingType = .no
        v.smartQuotesType = .no
        v.smartDashesType = .no
        v.keyboardType = .URL
        v.textContainerInset = UIEdgeInsets(top: 8, left: 0, bottom: 8, right: 0)
        v.delegate = context.coordinator
        return v
    }

    func updateUIView(_ v: UITextView, context: Context) {
        if v.text != text { v.text = text }
    }

    func makeCoordinator() -> Coordinator { Coordinator(text: $text) }

    final class Coordinator: NSObject, UITextViewDelegate {
        var text: Binding<String>
        init(text: Binding<String>) { self.text = text }
        func textViewDidChange(_ v: UITextView) { text.wrappedValue = v.text }
    }
}
