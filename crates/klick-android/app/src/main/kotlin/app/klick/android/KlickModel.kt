package app.klick.android

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject

data class Conn(val id: String, val name: String, val kind: String, val used: Double?, val total: Double?, val expire: Double?)
data class Srv(val name: String, val kind: String, val delay: Int?, val selected: Boolean)

/** Состояние окна из движка: подключение, подписки, серверы. Команды — [Engine.call], события — [Engine.events]. */
class KlickModel : ViewModel() {
    var ready by mutableStateOf(false)
    var vpn by mutableStateOf("off")
    var server by mutableStateOf<String?>(null)
    var since by mutableStateOf(0.0)
    var active by mutableStateOf<String?>(null)
    val connections = mutableStateListOf<Conn>()
    val servers = mutableStateListOf<Srv>()
    var pinging by mutableStateOf(false)
    var busy by mutableStateOf(false)
    var message by mutableStateOf<String?>(null)

    init {
        viewModelScope.launch {
            while (!KlickApp.ready) {
                KlickApp.startError?.let { message = "Движок не запустился: $it"; return@launch }
                delay(100)
            }
            ready = true
            reload()
            Engine.events.collect { onEvent(it) }
        }
    }

    private suspend fun reload() {
        runCatching { applyState(JSONObject(Engine.call("status"))) }
        runCatching { applySettings(JSONObject(Engine.call("settings"))) }
        // Подписки есть, а выбранной нет (первая добавленная): выбрать первую — иначе серверов не видно.
        if (active == null && connections.isNotEmpty()) {
            runCatching { Engine.call("select_connection", JSONObject().put("id", connections.first().id)) }
            runCatching { applySettings(JSONObject(Engine.call("settings"))) }
        }
        loadServers()
    }

    private suspend fun loadServers() {
        if (active == null) return servers.clear()
        try {
            applyServers(JSONArray(Engine.call("servers")))
        } catch (e: EngineError) {
            message = "Серверы: ${Texts.error(e.code)} (${e.code})"
        } catch (e: Exception) {
            message = "Серверы: ${e.message}"
        }
    }

    private suspend fun onEvent(ev: JSONObject) {
        when (ev.optString("ev")) {
            "state" -> ev.optJSONObject("state")?.let(::applyState)
            "settings" -> {
                runCatching { applySettings(JSONObject(Engine.call("settings"))) }
                loadServers()
            }
            "servers" -> loadServers()
            "notice" -> Texts.notice(ev.optString("code"), ev.optJSONObject("params"))?.let { message = it }
        }
    }

    private fun applyState(st: JSONObject) {
        vpn = st.optString("vpn", "off")
        server = st.str("server")
        since = st.optDouble("since", 0.0)
    }

    private fun applySettings(s: JSONObject) {
        active = s.str("activeConnection")
        val list = s.optJSONArray("connections") ?: JSONArray()
        connections.clear()
        for (i in 0 until list.length()) {
            val c = list.getJSONObject(i)
            val info = c.optJSONObject("info")
            connections += Conn(
                id = c.getString("id"), name = c.optString("name"), kind = c.optString("kind"),
                used = info?.optDouble("used")?.takeIf { !it.isNaN() },
                total = info?.optDouble("total")?.takeIf { !it.isNaN() && it > 0 },
                expire = info?.optDouble("expire")?.takeIf { !it.isNaN() && it > 0 },
            )
        }
    }

    private fun applyServers(list: JSONArray) {
        servers.clear()
        for (i in 0 until list.length()) {
            val s = list.getJSONObject(i)
            servers += Srv(s.optString("name"), s.optString("kind"), if (s.isNull("delay")) null else s.optInt("delay"), s.optBoolean("selected"))
        }
    }

    private fun run(block: suspend () -> Unit) {
        viewModelScope.launch {
            busy = true
            try {
                block()
            } catch (e: EngineError) {
                message = Texts.error(e.code)
            } catch (e: Exception) {
                message = e.message
            } finally {
                busy = false
            }
        }
    }

    fun connect() = run { Engine.call("connect") }
    fun disconnect() = run { Engine.call("disconnect") }

    fun add(source: String) = run {
        val args = JSONObject().put("source", source.trim()).put("name", JSONObject.NULL)
        Engine.call("add_connection", args)
        message = "Подписка добавлена"
        reload()
    }

    fun selectConnection(id: String) = run {
        Engine.call("select_connection", JSONObject().put("id", id))
        reload()
    }

    fun selectServer(name: String) = run {
        Engine.call("select_server", JSONObject().put("name", name))
        loadServers()
    }

    fun testLatency() {
        if (pinging) return
        viewModelScope.launch {
            pinging = true
            try {
                applyServers(JSONArray(Engine.call("test_latency")))
            } catch (e: EngineError) {
                message = Texts.error(e.code)
            } finally {
                pinging = false
            }
        }
    }
}

/** Строка из JSON: нет ключа, `null` или пусто — null (`optString` отдал бы текст «null»). */
fun JSONObject.str(key: String): String? = if (isNull(key)) null else optString(key).ifEmpty { null }
