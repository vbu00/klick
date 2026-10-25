import SwiftUI
import UIKit

/// Добавить подписку или сервер: ссылка, буфер обмена или QR-код (камера или фото) — как в веб-версии.
struct AddView: View {
    @EnvironmentObject var engine: Engine
    @Binding var tab: Int
    @State private var link = ""
    @State private var name = ""
    @State private var busy = false
    @State private var scanning = false
    @FocusState private var focused: Bool

    private var kind: String { Texts.kind(link) }
    private var ready: Bool { kind == "sub" || kind == "link" }

    var body: some View {
        KScreen(title: "Добавить") {
            VStack(alignment: .leading, spacing: 12) {
                TextField("https://panel.example/sub/… или vless://…", text: $link)
                    .font(.system(size: 15, design: .monospaced))
                    .foregroundColor(K.text)
                    .textInputAutocapitalization(.never)
                    .disableAutocorrection(true)
                    .keyboardType(.URL)
                    .submitLabel(.done)
                    .focused($focused)
                HStack(spacing: 8) {
                    Button {
                        if let text = UIPasteboard.general.string { link = text.trimmingCharacters(in: .whitespacesAndNewlines) }
                    } label: {
                        Label("Вставить", systemImage: "doc.on.clipboard")
                    }
                    .buttonStyle(PillButtonStyle())
                    Button {
                        scan()
                    } label: {
                        Label(scanning ? "Сканирую…" : "QR-код", systemImage: "qrcode.viewfinder")
                    }
                    .buttonStyle(PillButtonStyle())
                    .disabled(scanning)
                    Spacer()
                    if !link.isEmpty {
                        Button {
                            link = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill").font(.system(size: 18)).foregroundColor(K.dim2)
                        }
                        .accessibilityLabel("Очистить")
                    }
                }
            }
            .padding(16)
            .kCard()
            .padding(.top, 10)

            status.padding(.top, 12)

            TextField("Название — необязательно", text: $name)
                .font(.system(size: 15))
                .foregroundColor(K.text)
                .padding(.horizontal, 16)
                .frame(height: 52)
                .kCard(radius: 16)
                .padding(.top, 12)

            Button {
                focused = false
                Task { await add() }
            } label: {
                if busy { ProgressView().tint(K.onAccent) } else { Text("Добавить") }
            }
            .buttonStyle(WideButtonStyle(enabled: ready && !busy))
            .disabled(busy || !ready)
            .padding(.top, 12)

            KSection("Что можно вставить")
            VStack(spacing: 0) {
                info("globe", "Ссылка на подписку",
                     "https://… от Remnawave, Marzban и подобных — в форматах Clash, sing-box, Xray или списком ссылок; ключ Outline ssconf://. Даёт список серверов, лимит трафика и срок действия. Обновляется автоматически.")
                KDivider(inset: 64)
                info("link", "Одиночная конфигурация",
                     "vless://, vmess://, trojan://, ss://, hysteria2:// — прямое соединение с одним сервером.")
            }
            .kCard()
        }
    }

    /// Что распознано в поле: точка и пояснение, как в веб-версии.
    private var status: some View {
        HStack(alignment: .top, spacing: 12) {
            Circle().fill(kind == "bad" ? K.orange : ready ? K.accent : K.dim2).frame(width: 7, height: 7).padding(.top, 5)
            VStack(alignment: .leading, spacing: 3) {
                Text(statusTitle).font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                Text(hint).font(.system(size: 12)).foregroundColor(K.dim).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(16)
        .kCard(radius: 18)
    }

    private var statusTitle: String {
        switch kind {
        case "sub": return "Ссылка на подписку"
        case "link": return "Одиночный сервер"
        case "bad": return "Формат не распознан"
        default: return "Ожидаем ссылку"
        }
    }

    private var hint: String {
        switch kind {
        case "sub": return "Загрузим список серверов, лимит трафика и срок действия. Обновляется сама."
        case "link": return "Прямое соединение по этой ссылке."
        case "bad": return "Нужна ссылка https://… или vless://, vmess://, trojan://, ss://, hysteria2://."
        default: return "Скопируйте её у провайдера VPN и вставьте выше — тип определится сам. QR-код можно отсканировать камерой или выбрать из «Фото»."
        }
    }

    private func info(_ icon: String, _ title: String, _ text: String) -> some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: icon).font(.system(size: 16)).foregroundColor(K.dim)
                .frame(width: 34, height: 34)
                .background(K.elem.opacity(0.8), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(.system(size: 14, weight: .semibold)).foregroundColor(K.text)
                Text(text).font(.system(size: 12)).foregroundColor(K.dim).fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: 0)
        }
        .padding(16)
    }

    private func add() async {
        busy = true
        defer { busy = false }
        if await engine.add(source: link.trimmingCharacters(in: .whitespacesAndNewlines), name: name.trimmingCharacters(in: .whitespaces)) {
            link = ""
            name = ""
            tab = 0
        }
    }

    /// Сканер ждёт, пока человек отсканирует или закроет его, — не на главном потоке (Sources/klick-ios/QRScanner.swift).
    private func scan() {
        scanning = true
        Task.detached(priority: .userInitiated) {
            let raw = klickScanQr()
            let text = raw.map { String(cString: $0) }
            if let raw { free(raw) }
            await MainActor.run {
                scanning = false
                if let text { link = Texts.fromDeepLink(text) }
            }
        }
    }
}
