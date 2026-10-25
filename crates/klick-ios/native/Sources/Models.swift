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

struct Settings: Decodable, Equatable {
    var routing: String
    var connections: [Connection]
    var activeConnection: String?
    var restoreOnLogon: Bool?
    var russiaDirect: RussiaDirect?
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
}

struct Traffic: Equatable {
    var up: Double = 0
    var down: Double = 0
}

struct Toast: Identifiable, Equatable {
    let id = UUID()
    var text: String
    var bad = false
}
