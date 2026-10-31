package app.klick.android

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

// Модели — как у iOS (crates/klick-ios/native/Sources/Models.swift); поля движка — snake_case (klick-proto).

data class SubInfo(
    val upload: Double, val download: Double, val total: Double?, val expire: Double?,
    val announce: String?, val supportUrl: String?, val pageUrl: String?,
) {
    val used get() = upload + download
}

data class Conn(val id: String, val name: String, val kind: String, val info: SubInfo?, val selectedServer: String?)
data class Srv(val name: String, val kind: String, val delay: Int?, val selected: Boolean, val speed: Double?)
data class Appearance(val theme: String = "system", val base: String = "graphite", val accent: String = "#30d158", val glass: Boolean = false)
data class StateView(val vpn: String, val routing: String?, val server: String?, val since: Double?, val error: String?, val attempt: List<Int>, val connectionId: String?)
data class SpeedView(val server: String?, val down: Double?, val up: Double?)
data class Target(val kind: String, val value: String)
data class Rule(val target: Target, val route: String?, val enabled: Boolean)
data class Service(val id: String, val name: String, val domains: List<String>, val cidrs: List<String>)
data class LiveConn(val host: String, val route: String, val upload: Double, val download: Double)
data class Failure(val host: String, val route: String, val error: String, val at: Double)
data class IpColumn(
    val ipv4: String?, val ipv6: String?, val country: String?, val countryCode: String?, val city: String?, val provider: String?,
    val reverseDns: String?, val vpnDetected: Boolean?, val error: String?,
)
data class IpReport(val viaVpn: IpColumn?, val direct: IpColumn?, val ipv6Leak: Boolean?, val dnsProtected: Boolean?, val checkedAt: Double)

private fun JSONArray?.strings(): List<String> = if (this == null) emptyList() else List(length()) { optString(it) }
private fun JSONObject.bool(key: String): Boolean? = if (isNull(key)) null else optBoolean(key)

private fun parseRules(a: JSONArray?): List<Rule> = if (a == null) emptyList() else List(a.length()) { i ->
    val r = a.getJSONObject(i)
    val t = r.optJSONObject("target") ?: JSONObject()
    Rule(Target(t.optString("kind"), t.optString("value")), r.str("route"), r.optBoolean("enabled", true))
}

private fun ipColumn(o: JSONObject?): IpColumn? = o?.let {
    IpColumn(it.str("ipv4"), it.str("ipv6"), it.str("country"), it.str("country_code"), it.str("city"), it.str("provider"),
        it.str("reverse_dns"), it.bool("vpn_detected"), it.str("error"))
}
data class Toast(val title: String, val sub: String? = null, val tone: String = "ok", val id: String = UUID.randomUUID().toString())

/** Скорость за последние 60 секунд: байт/с сейчас, история в Мбит/с, сколько всего. */
data class Traffic(
    val up: Double = 0.0, val down: Double = 0.0,
    val upHist: List<Double> = List(HIST) { 0.0 }, val downHist: List<Double> = List(HIST) { 0.0 },
    val upSum: Double = 0.0, val downSum: Double = 0.0,
) {
    fun push(up: Double, down: Double) = Traffic(
        up, down, upHist.drop(1) + up * 8 / 1e6, downHist.drop(1) + down * 8 / 1e6, upSum + up, downSum + down,
    )

    companion object { const val HIST = 60 }
}

/** Строка из JSON: нет ключа, `null` или пусто — null (`optString` отдал бы текст «null»). */
fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key).ifEmpty { null }
fun JSONObject.num(key: String): Double? = if (isNull(key)) null else optDouble(key).takeIf { !it.isNaN() }

/**
 * Состояние окна из движка — как `Engine` у iOS: подключение, подписки, серверы, трафик, замер скорости,
 * уведомления. Команды — [Engine.call], события — [Engine.events].
 */
class KlickModel : ViewModel() {
    var up by mutableStateOf(false)
    var startError by mutableStateOf<String?>(null)
    var state by mutableStateOf<StateView?>(null)
    var connections by mutableStateOf<List<Conn>>(emptyList())
    var activeId by mutableStateOf<String?>(null)
    var routing by mutableStateOf("selected")
    var serversByCountry by mutableStateOf(false)
    var appearance by mutableStateOf(Appearance())
    var onServerDown by mutableStateOf("reconnect")
    var restoreOnLogon by mutableStateOf(false)
    var autoUpdate by mutableStateOf(true)
    /** Мой список: в «VPN для всего» — что идёт напрямую, в «VPN для выбранного» — что через VPN. */
    var listAllVpn by mutableStateOf<List<Rule>>(emptyList())
    var listSelected by mutableStateOf<List<Rule>>(emptyList())
    var blockedPreset by mutableStateOf(true)
    var ruDomains by mutableStateOf(false)
    var ruIps by mutableStateOf(false)
    /** Каталог сервисов: грузится один раз. */
    var catalog by mutableStateOf<List<Service>>(emptyList())

    val list get() = if (routing == "all_vpn") listAllVpn else listSelected
    /** Серверы по подпискам: свою можно смотреть, не переключаясь на неё. */
    val servers = mutableStateMapOf<String, List<Srv>>()
    /** Подписки, где задержку уже мерили: тогда пустая задержка — «нет ответа», а не «ещё не мерили». */
    val tested = mutableStateMapOf<String, Boolean>()
    var traffic by mutableStateOf(Traffic())
    var pinging by mutableStateOf(false)
    var speed by mutableStateOf<SpeedView?>(null)
    var speedTesting by mutableStateOf(false)
    /** Нажали кнопку питания, движок ещё не ответил: `on` — включаю, `off` — выключаю. */
    var pending by mutableStateOf<String?>(null)
    var toast by mutableStateOf<Toast?>(null)
    /** Показать на главной это подключение («Уже добавлено»). */
    var focusConnection by mutableStateOf<String?>(null)

    val vpn get() = state?.vpn ?: "off"
    val isRunning get() = vpn in setOf("connecting", "connected", "reconnecting", "server_down")

    init {
        viewModelScope.launch {
            while (!KlickApp.ready) {
                KlickApp.startError?.let { startError = it; return@launch }
                delay(100)
            }
            up = true
            reload()
            Engine.events.collect { onEvent(it) }
        }
    }

    // ── Загрузка ─────────────────────────────────────────────

    suspend fun reload() {
        runCatching { applyState(JSONObject(Engine.call("status"))) }
        reloadSettings()
        // Подписки есть, а выбранной нет (первая добавленная): выбрать первую — иначе серверов не видно.
        if (activeId == null && connections.isNotEmpty()) {
            runCatching { Engine.call("select_connection", JSONObject().put("id", connections.first().id)) }
            reloadSettings()
        }
        loadServers()
    }

    private suspend fun reloadSettings() {
        runCatching { applySettings(JSONObject(Engine.call("settings"))) }
    }

    private suspend fun loadServers() {
        val id = activeId ?: return
        runCatching { servers[id] = parseServers(JSONArray(Engine.call("servers"))) }
    }

    /** Серверы любой подписки — не переключаясь на неё. */
    suspend fun viewServers(id: String) {
        if (id == activeId) return loadServers()
        runCatching { servers[id] = parseServers(JSONArray(Engine.call("connection_servers", JSONObject().put("id", id)))) }
    }

    private suspend fun onEvent(ev: JSONObject) {
        when (ev.optString("ev")) {
            "state" -> ev.optJSONObject("state")?.let { raw ->
                val before = state
                applyState(raw)
                if (vpn != "connected" && traffic != Traffic()) traffic = Traffic()
                if (state?.connectionId != before?.connectionId) reload()
            }
            "traffic" -> traffic = traffic.push(ev.optDouble("up", 0.0), ev.optDouble("down", 0.0))
            "settings" -> reloadSettings()
            "servers" -> ev.str("connection")?.let { id ->
                if (id == activeId) tested[id] = true
                viewServers(id)
            }
            "notice" -> Texts.notice(ev.optString("code"), ev.optJSONObject("params"))?.let {
                toast = Toast(it, tone = if (Texts.noticeIsBad(ev.optString("code"))) "bad" else "ok")
            }
        }
    }

    private fun applyState(st: JSONObject) {
        val attempt = st.optJSONArray("attempt")?.let { a -> List(a.length()) { a.optInt(it) } }.orEmpty()
        state = StateView(
            vpn = st.optString("vpn", "off"), routing = st.str("routing"), server = st.str("server"),
            since = st.num("since"), error = st.str("error"), attempt = attempt,
            connectionId = st.optJSONObject("connection")?.str("id"),
        )
        st.str("routing")?.let { routing = it }
    }

    private fun applySettings(s: JSONObject) {
        activeId = s.str("active_connection")
        routing = s.str("routing") ?: routing
        serversByCountry = s.optBoolean("servers_by_country", false)
        onServerDown = s.str("on_server_down") ?: "reconnect"
        restoreOnLogon = s.optBoolean("restore_on_logon", false)
        autoUpdate = s.optBoolean("auto_update", true)
        s.optJSONObject("lists")?.let {
            listAllVpn = parseRules(it.optJSONArray("all_vpn"))
            listSelected = parseRules(it.optJSONArray("selected"))
        }
        blockedPreset = s.optBoolean("blocked_preset", true)
        s.optJSONObject("russia_direct")?.let {
            ruDomains = it.optBoolean("domains")
            ruIps = it.optBoolean("ips")
        }
        s.optJSONObject("appearance")?.let { a ->
            appearance = Appearance(a.str("theme") ?: "system", a.str("base") ?: "graphite", a.str("accent") ?: "#30d158", a.optBoolean("glass"))
        }
        val list = s.optJSONArray("connections") ?: JSONArray()
        connections = List(list.length()) { i ->
            val c = list.getJSONObject(i)
            val info = c.optJSONObject("info")?.let {
                SubInfo(it.optDouble("upload", 0.0), it.optDouble("download", 0.0), it.num("total")?.takeIf { t -> t > 0 },
                    it.num("expire")?.takeIf { e -> e > 0 }, it.str("announce"), it.str("support_url"), it.str("page_url"))
            }
            Conn(c.getString("id"), c.optString("name"), c.optString("kind"), info, c.str("selected_server"))
        }
    }

    private fun parseServers(list: JSONArray) = List(list.length()) { i ->
        val s = list.getJSONObject(i)
        Srv(s.optString("name"), s.optString("kind"), if (s.isNull("delay")) null else s.optInt("delay"), s.optBoolean("selected"), s.num("speed"))
    }

    // ── Действия ─────────────────────────────────────────────

    fun show(e: Throwable) {
        val code = (e as? EngineError)?.code ?: "unknown"
        toast = Toast(Texts.error(code), tone = "bad")
    }

    private fun launch(block: suspend () -> Unit) = viewModelScope.launch {
        try {
            block()
        } catch (e: Exception) {
            show(e)
        }
    }

    /** Кнопка питания. [viewing] — подписка на экране: без VPN она становится активной и подключается. */
    fun power(viewing: String?) = viewModelScope.launch {
        if (pending != null) return@launch
        pending = if (isRunning) "off" else "on"
        try {
            if (isRunning) {
                Engine.call("disconnect")
            } else {
                if (viewing != null && viewing != activeId) Engine.call("select_connection", JSONObject().put("id", viewing))
                Engine.call("connect")
            }
        } catch (e: Exception) {
            show(e)
        } finally {
            pending = null
        }
        reload()
    }

    suspend fun selectConnection(id: String): Boolean = try {
        Engine.call("select_connection", JSONObject().put("id", id))
        reload()
        true
    } catch (e: Exception) {
        show(e)
        false
    }

    fun selectServer(name: String) = launch {
        Engine.call("select_server", JSONObject().put("name", name))
        loadServers()
    }

    fun testLatency() = viewModelScope.launch {
        val id = activeId ?: return@launch
        if (pinging) return@launch
        pinging = true
        try {
            servers[id] = parseServers(JSONArray(Engine.call("test_latency")))
            tested[id] = true
        } catch (e: Exception) {
            show(e)
        } finally {
            pinging = false
        }
    }

    fun speedTest() = viewModelScope.launch {
        if (speedTesting) return@launch
        speedTesting = true
        try {
            val r = JSONObject(Engine.call("speed_test"))
            speed = SpeedView(r.str("server"), r.num("down"), r.num("up"))
            loadServers()
        } catch (e: Exception) {
            show(e)
        } finally {
            speedTesting = false
        }
    }

    suspend fun add(source: String, name: String): Boolean = try {
        val args = JSONObject().put("source", source).put("name", if (name.isEmpty()) JSONObject.NULL else name)
        Engine.call("add_connection", args)
        toast = Toast("Подключение добавлено", "Проверьте задержку серверов")
        reload()
        true
    } catch (e: EngineError) {
        if (e.code == "conn.exists") {
            // Эта ссылка уже добавлена: показать то подключение вместо второго такого же.
            toast = Toast("Уже добавлено", e.params.str("name"), tone = "dim")
            focusConnection = e.params.str("id")
            true
        } else {
            show(e)
            false
        }
    } catch (e: Exception) {
        show(e)
        false
    }

    /** Список ссылок или конфигурация текстом — как файл; расширение — чтобы точка в названии не съела хвост. */
    suspend fun importText(name: String, content: String) = importFile("$name.txt", content, "Подключение добавлено", name)

    suspend fun importFile(fileName: String, content: String, title: String = "Файл импортирован", sub: String? = null): Boolean = try {
        Engine.call("import_file", JSONObject().put("file_name", fileName).put("content", content))
        toast = Toast(title, sub ?: fileName)
        reload()
        true
    } catch (e: Exception) {
        show(e)
        false
    }

    suspend fun refresh(id: String) {
        try {
            Engine.call("refresh_connection", JSONObject().put("id", id))
            toast = Toast("Подписка обновлена")
        } catch (e: Exception) {
            show(e)
        }
        reload()
    }

    /** Настройки движка: `{"on_server_down": "fastest"}` и т. п.; ответ — свежие настройки. */
    fun setPrefs(prefs: JSONObject) = launch {
        applySettings(JSONObject(Engine.call("set_preferences", JSONObject().put("prefs", prefs))))
    }

    /** Тема — в движке, как у веб-версии; применяется сразу, не дожидаясь ответа. */
    fun setAppearance(a: Appearance) {
        appearance = a
        setPrefs(JSONObject().put("appearance", JSONObject().put("theme", a.theme).put("base", a.base).put("accent", a.accent).put("glass", a.glass)))
    }

    fun setRouting(routing: String) = launch {
        Engine.call("set_routing", JSONObject().put("routing", routing))
        reloadSettings()
    }

    /** «О приложении»: версия kl!ck и ядра. */
    suspend fun about(): Pair<String?, String?> = runCatching {
        val a = JSONObject(Engine.call("about"))
        a.str("version") to a.str("core_version")
    }.getOrDefault(null to null)

    // ── Соединение: мой список, каталог, сейчас в сети ───────

    suspend fun loadCatalog() {
        if (catalog.isNotEmpty()) return
        runCatching {
            val a = JSONObject(Engine.call("catalog")).optJSONArray("services") ?: JSONArray()
            catalog = List(a.length()) { i ->
                val s = a.getJSONObject(i)
                Service(s.optString("id"), s.optString("name"), s.optJSONArray("domains").strings(), s.optJSONArray("cidrs").strings())
            }
        }
    }

    suspend fun connections(): List<LiveConn>? = runCatching {
        val a = JSONArray(Engine.call("connections"))
        List(a.length()) { i -> a.getJSONObject(i).let { LiveConn(it.optString("host"), it.optString("route"), it.optDouble("upload", 0.0), it.optDouble("download", 0.0)) } }
    }.getOrNull()

    suspend fun failures(): List<Failure>? = runCatching {
        val a = JSONArray(Engine.call("failures"))
        List(a.length()) { i -> a.getJSONObject(i).let { Failure(it.optString("host"), it.optString("route"), it.optString("error"), it.optDouble("at", 0.0)) } }
    }.getOrNull()

    suspend fun ipCheck(): IpReport? = try {
        val r = JSONObject(Engine.call("ip_check"))
        IpReport(ipColumn(r.optJSONObject("via_vpn")), ipColumn(r.optJSONObject("direct")), r.bool("ipv6_leak"), r.bool("dns_protected"), r.optDouble("checked_at", 0.0))
    } catch (e: Exception) {
        show(e)
        null
    }

    private fun ruleJson(t: Target, route: String) = JSONObject().put("target", JSONObject().put("kind", t.kind).put("value", t.value)).put("route", route).put("enabled", true)

    /** Добавить правила в список положения; сколько добавилось. */
    suspend fun addRules(position: String, rules: List<Pair<Target, String>>): Int {
        var added = 0
        var first: Exception? = null
        for ((t, route) in rules) {
            try {
                Engine.call("list_add", JSONObject().put("position", position).put("rule", ruleJson(t, route)))
                added++
            } catch (e: Exception) {
                if (first == null) first = e
            }
        }
        first?.let(::show)
        reloadSettings()
        return added
    }

    fun removeRule(position: String, index: Int) = launch {
        Engine.call("list_remove", JSONObject().put("position", position).put("index", index))
        toast = Toast("Правило удалено", tone = "dim")
        reloadSettings()
    }

    fun setRuleEnabled(position: String, index: Int, enabled: Boolean) = launch {
        Engine.call("list_set_enabled", JSONObject().put("position", position).put("index", index).put("enabled", enabled))
        reloadSettings()
    }

    /** Один тумблер «через VPN»: правило для цели в мой список текущего положения (live.ts: planToggle). */
    fun routeTarget(target: Target, vpn: Boolean, label: String) = launch {
        val position = routing
        val l = list
        val route = if (vpn) "vpn" else "direct"
        val index = l.indexOfFirst { it.target.kind == target.kind && it.target.value.equals(target.value, ignoreCase = true) }
        if (index >= 0) {
            val r = l[index]
            if (r.route == route && r.enabled) return@launch
            if (r.route != route) Engine.call("list_set_route", JSONObject().put("position", position).put("index", index).put("route", route))
            if (!r.enabled) Engine.call("list_set_enabled", JSONObject().put("position", position).put("index", index).put("enabled", true))
        } else {
            Engine.call("list_add", JSONObject().put("position", position).put("rule", ruleJson(target, route)))
        }
        toast = Toast("$label → ${if (vpn) "через VPN" else "напрямую"}", "Правило в «${if (position == "selected") "Через VPN" else "Напрямую"} · мой список»")
        reloadSettings()
    }

    data class LogLine(val at: Double, val level: String, val text: String)

    suspend fun log(): List<LogLine> = runCatching {
        val a = JSONArray(Engine.call("log"))
        List(a.length()) { i -> a.getJSONObject(i).let { LogLine(it.optDouble("at", 0.0), it.optString("level"), it.optString("text")) } }
    }.getOrDefault(emptyList())

    /** Текст для поддержки: версии, режим и журнал — без ссылок и адресов сайтов. */
    suspend fun report(): String? = runCatching { Engine.call("report").let { if (it.startsWith("\"")) JSONArray("[$it]").getString(0) else it } }.getOrNull()

    fun remove(id: String) = launch {
        try {
            Engine.call("remove_connection", JSONObject().put("id", id))
        } finally {
            reload()
        }
    }
}
