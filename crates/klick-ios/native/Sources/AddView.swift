import SwiftUI
import UIKit

/// Добавить подписку или сервер: ссылка, буфер обмена или QR-код (камера или фото).
struct AddView: View {
    @EnvironmentObject var engine: Engine
    @Binding var tab: Int
    @State private var link = ""
    @State private var name = ""
    @State private var busy = false
    @State private var scanning = false

    private var kind: String { Texts.kind(link) }

    var body: some View {
        NavigationView {
            Form {
                Section {
                    TextField("https://… или vless://…", text: $link)
                        .textInputAutocapitalization(.never)
                        .disableAutocorrection(true)
                        .keyboardType(.URL)
                    HStack {
                        Button {
                            if let text = UIPasteboard.general.string { link = text.trimmingCharacters(in: .whitespacesAndNewlines) }
                        } label: {
                            Label("Вставить", systemImage: "doc.on.clipboard")
                        }
                        .buttonStyle(.borderless)
                        Spacer()
                        Button {
                            scan()
                        } label: {
                            Label(scanning ? "Сканирую…" : "QR-код", systemImage: "qrcode.viewfinder")
                        }
                        .buttonStyle(.borderless)
                        .disabled(scanning)
                    }
                } header: {
                    Text("Ссылка")
                } footer: {
                    Text(hint)
                }
                Section {
                    TextField("Необязательно", text: $name)
                } header: {
                    Text("Название")
                }
                Section {
                    Button {
                        Task { await add() }
                    } label: {
                        HStack {
                            Spacer()
                            if busy { ProgressView() } else { Text("Добавить").fontWeight(.semibold) }
                            Spacer()
                        }
                    }
                    .disabled(busy || !(kind == "sub" || kind == "link"))
                }
            }
            .navigationTitle("Добавить")
        }
        .navigationViewStyle(.stack)
    }

    private var hint: String {
        switch kind {
        case "sub": return "Подписка: загрузим список серверов, лимит трафика и срок действия. Обновляется сама."
        case "link": return "Одиночный сервер: прямое соединение по этой ссылке."
        case "bad": return "Формат не распознан. Нужна ссылка https://… или vless://, vmess://, trojan://, ss://, hysteria2://."
        default: return "Ссылку на подписку даёт ваш VPN-сервис. QR-код можно отсканировать камерой или выбрать из «Фото»."
        }
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
