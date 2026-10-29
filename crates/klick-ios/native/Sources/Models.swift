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

/// Правило списка: сервис, сайт или IP; `route` — `vpn`, `direct` или `block`.
struct Rule: Decodable, Equatable {
    struct Target: Decodable, Equatable, Hashable {
        var kind: String
        var value: String
    }
    var target: Target
    var route: String?
    var enabled: Bool?

    /// JSON для `list_add`.
    static func json(_ target: Target, route: String) -> [String: Any] {
        ["target": ["kind": target.kind, "value": target.value], "route": route, "enabled": true]
    }
}

/// Ответ `catalog`: `{services: […]}`.
struct Catalog: Decodable {
    var services: [Service]
}

/// Сервис каталога (`catalog`): название, домены и сети — для «Добавить» и «Сейчас в сети».
struct Service: Decodable, Identifiable, Equatable {
    var id: String
    var name: String
    var domains: [String]
    var cidrs: [String]

    enum CodingKeys: String, CodingKey { case id, name, domains, cidrs }

    init(from d: Decoder) throws {
        let c = try d.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        name = try c.decode(String.self, forKey: .name)
        domains = (try? c.decode([String].self, forKey: .domains)) ?? []
        cidrs = (try? c.decode([String].self, forKey: .cidrs)) ?? []
    }
}

/// Соединение ядра сейчас (`connections`).
struct LiveConn: Decodable, Equatable {
    var host: String
    var process: String?
    var route: String
    var upload: Double
    var download: Double
}

/// Неудачное соединение (`failures`) — «Не открывается?».
struct Failure: Decodable, Equatable {
    var host: String
    var route: String
    var error: String
    var at: Double
}

/// Колонка «Как вас видят сайты».
struct IpColumn: Decodable, Equatable {
    var ipv4: String?
    var ipv6: String?
    var country: String?
    var countryCode: String?
    var city: String?
    var provider: String?
    var asn: Int?
    var reverseDns: String?
    var vpnDetected: Bool?
    var error: String?
}

struct IpReport: Decodable, Equatable {
    var viaVpn: IpColumn?
    var direct: IpColumn
    var ipv6Leak: Bool?
    var dnsProtected: Bool?
    var checkedAt: Double
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
    var appearance: Appearance?
}

/// Оформление: тема (`system`, `light`, `dark`, `custom`), основа и акцент своей темы, стекло.
struct Appearance: Codable, Equatable {
    var theme: String
    var base: String
    var accent: String
    var glass: Bool

    static let standard = Appearance(theme: "system", base: "graphite", accent: "#30d158", glass: false)

    var json: [String: Any] { ["theme": theme, "base": base, "accent": accent, "glass": glass] }
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

/// Настройки только этого iPhone (не движка): что показывать поверх приложения и в Dynamic Island.
enum Prefs {
    static let liveActivity = "pref.liveActivity"
    static let toasts = "pref.toasts"
    static let haptics = "pref.haptics"
    /// Dynamic Island: когда (`never`, `trouble`, `always`) и что слева (`logo`, `flag`, `dot`) и справа
    /// (`none`, `ping`, `server`, `time`) в режиме «всегда».
    static let island = "pref.island"
    static let islandLeft = "pref.islandLeft"
    static let islandRight = "pref.islandRight"

    /// Не трогали — включено.
    static func on(_ key: String) -> Bool { UserDefaults.standard.object(forKey: key) as? Bool ?? true }

    /// Режим острова; до него был переключатель «Dynamic Island» — выключенный значит «никогда».
    static var islandMode: String { UserDefaults.standard.string(forKey: island) ?? (on(liveActivity) ? "trouble" : "never") }
    static var left: String { UserDefaults.standard.string(forKey: islandLeft) ?? "logo" }
    static var right: String { UserDefaults.standard.string(forKey: islandRight) ?? "server" }
}
