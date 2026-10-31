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

    fun remove(id: String) = launch {
        try {
            Engine.call("remove_connection", JSONObject().put("id", id))
        } finally {
            reload()
        }
    }
}
