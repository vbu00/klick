import Darwin
import Foundation

/// Канал к службе kl!ck: Unix-сокет, одна строка JSON на сообщение (crates/klick-proto). Окно шлёт
/// `{"id","cmd","args"}`, служба отвечает `{"t":"res","id","ok","data"|"error"}`, а после `subscribe` —
/// ещё и события `{"t":"event","ev",…}`. Служба перезапустилась — канал поднимается сам.
final class ServiceLink: @unchecked Sendable {
    /// Ответ: получилось ли и JSON — данные или `{code, params}`.
    typealias Reply = (ok: Bool, data: Data)

    /// Рабочая служба (создать файл в /var/run может только root — подменить её нельзя) и служба для разработки.
    static let paths = ["/var/run/klick.sock", "/tmp/klick-dev.sock"]

    var onEvent: ((Data) -> Void)?
    /// Канал поднялся (true) или упал (false).
    var onLink: ((Bool) -> Void)?

    private let lock = NSLock()
    private var fd: Int32 = -1
    private var nextId: UInt64 = 1
    private var waiting: [UInt64: CheckedContinuation<Reply, Never>] = [:]
    private var running = false

    /// Поднять канал и держать его: упал — через 2 секунды снова.
    func start() {
        lock.lock()
        defer { lock.unlock() }
        guard !running else { return }
        running = true
        let t = Thread { [weak self] in self?.loop() }
        t.name = "klick.link"
        t.start()
    }

    func call(_ cmd: String, _ args: [String: Any]?) async -> Reply {
        await withCheckedContinuation { (c: CheckedContinuation<Reply, Never>) in
            lock.lock()
            guard fd >= 0 else {
                lock.unlock()
                c.resume(returning: (false, Self.error("service.unreachable")))
                return
            }
            let id = nextId
            nextId += 1
            var req: [String: Any] = ["id": id, "cmd": cmd]
            if let args { req["args"] = args }
            guard var line = try? JSONSerialization.data(withJSONObject: req) else {
                lock.unlock()
                c.resume(returning: (false, Self.error("unknown")))
                return
            }
            line.append(0x0A)
            waiting[id] = c
            let ok = Self.writeAll(fd, line)
            lock.unlock()
            if !ok { fail(id) }
        }
    }

    // MARK: Внутри

    private func loop() {
        while true {
            if let sock = Self.open() {
                lock.lock()
                fd = sock
                lock.unlock()
                onLink?(true)
                read(sock)
                lock.lock()
                fd = -1
                let pending = waiting
                waiting = [:]
                lock.unlock()
                close(sock)
                for (_, c) in pending { c.resume(returning: (false, Self.error("service.unreachable"))) }
                onLink?(false)
            }
            Thread.sleep(forTimeInterval: 2)
        }
    }

    /// Читать строки, пока канал жив.
    private func read(_ sock: Int32) {
        var buffer = Data()
        var chunk = [UInt8](repeating: 0, count: 65536)
        while true {
            let n = Darwin.read(sock, &chunk, chunk.count)
            if n <= 0 { return }
            buffer.append(chunk, count: n)
            while let nl = buffer.firstIndex(of: 0x0A) {
                let line = buffer[buffer.startIndex..<nl]
                buffer.removeSubrange(buffer.startIndex...nl)
                if !line.isEmpty { handle(Data(line)) }
            }
        }
    }

    private func handle(_ line: Data) {
        guard let obj = (try? JSONSerialization.jsonObject(with: line)) as? [String: Any] else { return }
        if obj["t"] as? String == "event" {
            onEvent?(line)
            return
        }
        guard let id = (obj["id"] as? NSNumber)?.uint64Value else { return }
        lock.lock()
        let c = waiting.removeValue(forKey: id)
        lock.unlock()
        let ok = obj["ok"] as? Bool ?? false
        let payload: Any = ok ? (obj["data"] ?? NSNull()) : (obj["error"] ?? ["code": "unknown"])
        let data = (try? JSONSerialization.data(withJSONObject: payload, options: .fragmentsAllowed)) ?? Data("null".utf8)
        c?.resume(returning: (ok, data))
    }

    private func fail(_ id: UInt64) {
        lock.lock()
        let c = waiting.removeValue(forKey: id)
        lock.unlock()
        c?.resume(returning: (false, Self.error("service.unreachable")))
    }

    private static func error(_ code: String) -> Data {
        Data(#"{"code":"\#(code)"}"#.utf8)
    }

    private static func writeAll(_ fd: Int32, _ data: Data) -> Bool {
        data.withUnsafeBytes { raw -> Bool in
            guard var p = raw.baseAddress else { return false }
            var left = raw.count
            while left > 0 {
                let n = write(fd, p, left)
                if n <= 0 { return false }
                p += n
                left -= n
            }
            return true
        }
    }

    /// Первый сокет, который отвечает.
    private static func open() -> Int32? {
        for path in paths where FileManager.default.fileExists(atPath: path) {
            let fd = socket(AF_UNIX, SOCK_STREAM, 0)
            if fd < 0 { continue }
            var addr = sockaddr_un()
            addr.sun_family = sa_family_t(AF_UNIX)
            let bytes = Array(path.utf8CString)
            withUnsafeMutableBytes(of: &addr.sun_path) { buf in
                for (i, b) in bytes.enumerated() where i < buf.count { buf[i] = UInt8(bitPattern: b) }
            }
            let ok = withUnsafePointer(to: &addr) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) }
            }
            if ok == 0 { return fd }
            close(fd)
        }
        return nil
    }
}
