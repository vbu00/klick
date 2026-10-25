import Foundation

// Ответы движка (JSON klick-proto). Ключи — snake_case, читаются с convertFromSnakeCase. Поля, которых
// у этой версии интерфейса нет, просто не читаются; лишние — необязательные.

struct SubInfo: Decodable, Equatable {
    var upload: Double?
    var download: Double?
    var total: Double?
    var expire: Double?
    var announce: String?
    var supportUrl: String?
    var pageUrl: String?

    var used: Double { (upload ?? 0) + (download ?? 0) }
}

struct ConnectionView: Decodable, Equatable {
    var id: String
    var name: String
    var info: SubInfo?
}

/// Что сейчас с VPN: `off`, `connecting`, `connected`, `reconnecting`, `server_down`, `error`.
struct StateView: Decodable, Equatable {
    var vpn: String
    var mode: String?
    var routing: String?
    var connection: ConnectionView?
    var server: String?
    var since: Double?
    var error: String?
    var update: String?
    /// Переподключение: попытка и сколько всего.
    var attempt: [Int]?
}

struct Connection: Decodable, Identifiable, Equatable {
    var id: String
    var name: String
    var kind: String
    var info: SubInfo?
    var selectedServer: String?
    var updatedAt: Double?
}

struct RussiaDirect: Decodable, Equatable {
    var domains: Bool
    var ips: Bool
}

/// Правило списка: сервис, сайт или IP.
struct Rule: Decodable, Equatable {
    struct Target: Decodable, Equatable {
        var kind: String
        var value: String
    }
    var target: Target
    var enabled: Bool?
}

struct Lists: Decodable, Equatable {
    var allVpn: [Rule]
    var selected: [Rule]
}

struct Settings: Decodable, Equatable {
    var routing: String
    var lists: Lists?
    var blockedPreset: Bool?
    var connections: [Connection]
    var activeConnection: String?
    var restoreOnLogon: Bool?
    var russiaDirect: RussiaDirect?
    var onServerDown: String?
    var autoUpdate: Bool?
    var serversByCountry: Bool?
}

/// Запись журнала службы.
struct LogLine: Decodable, Identifiable, Equatable {
    var at: Double
    var level: String
    var text: String
    var id: String { "\(at)\(text)" }
}

struct Server: Decodable, Identifiable, Equatable {
    var name: String
    var kind: String
    var delay: Int?
    var selected: Bool
    var speed: Double?

    var id: String { name }
}

/// «Проверить скорость», Мбит/с.
struct SpeedView: Decodable, Equatable {
    var server: String?
    var down: Double?
    var up: Double?
}

struct About: Decodable {
    var version: String
    var coreVersion: String?
    var os: String?
    var dataDir: String?
}

/// Скорость, история за 60 секунд (Мбит/с) и сколько прошло, пока VPN включён (store.tsx: trafficReducer).
struct Traffic: Equatable {
    static let hist = 60
    var up: Double = 0
    var down: Double = 0
    var upHist = [Double](repeating: 0, count: Traffic.hist)
    var downHist = [Double](repeating: 0, count: Traffic.hist)
    var upSum: Double = 0
    var downSum: Double = 0

    mutating func push(up: Double, down: Double) {
        self.up = up
        self.down = down
        upHist = Array(upHist.dropFirst()) + [up * 8 / 1e6]
        downHist = Array(downHist.dropFirst()) + [down * 8 / 1e6]
        upSum += up
        downSum += down
    }
}

/// Уведомление внизу: заголовок, подпись и точка — `ok`, `warn`, `bad`, `dim`.
struct Toast: Identifiable, Equatable {
    let id = UUID()
    var title: String
    var sub: String?
    var tone = "ok"

    init(_ title: String, _ sub: String? = nil, tone: String = "ok") {
        self.title = title
        self.sub = sub
        self.tone = tone
    }

    init(text: String, bad: Bool = false) {
        self.init(text, tone: bad ? "bad" : "ok")
    }
}
