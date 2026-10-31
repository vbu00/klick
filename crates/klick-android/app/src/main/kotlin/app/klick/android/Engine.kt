package app.klick.android

import android.util.Log
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.SharedFlow
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/** Ошибка движка: `code` — как в klick-proto (`service.unreachable`, `sub.refused`…), `params` — подробности. */
class EngineError(val code: String, val params: JSONObject) : Exception(code)

/**
 * Движок kl!ck (Rust, `crates/klick-android/src/lib.rs`): те же команды, что у окна на компьютере и у
 * iOS, — JSON klick-proto `{"cmd": "...", "args": {...}}`, и события `{"ev": "state" | "traffic" | ...}`.
 */
object Engine {
    init {
        // Ядро первым: движок берёт его функции через dlopen.
        System.loadLibrary("klickcore")
        System.loadLibrary("klick_android")
    }

    @JvmStatic external fun nativeStart(data: String, group: String, resources: String): String?
    @JvmStatic external fun nativeCall(request: String, id: Long)
    @JvmStatic external fun nativeSubscribe(): Boolean
    @JvmStatic external fun nativeTunnelChanged(connected: Boolean, byUser: Boolean)

    private val nextId = AtomicLong()
    private val pending = ConcurrentHashMap<Long, CompletableDeferred<Result<String>>>()
    private val _events = MutableSharedFlow<JSONObject>(extraBufferCapacity = 64)
    val events: SharedFlow<JSONObject> = _events

    @JvmStatic
    fun onReply(id: Long, json: String, ok: Boolean) {
        val result = if (ok) {
            Result.success(json)
        } else {
            val obj = runCatching { JSONObject(json) }.getOrElse { JSONObject() }
            Result.failure(EngineError(obj.optString("code", "unknown"), obj.optJSONObject("params") ?: JSONObject()))
        }
        pending.remove(id)?.complete(result)
    }

    @JvmStatic
    fun onEvent(json: String) {
        Log.i(TAG, "событие ${json.take(160)}")
        runCatching { JSONObject(json) }.onSuccess { _events.tryEmit(it) }
    }

    /** Команда движку; ответ — JSON-текст результата. Ошибка — [EngineError]. */
    suspend fun call(cmd: String, args: JSONObject? = null): String {
        val request = JSONObject().put("cmd", cmd)
        if (args != null) request.put("args", args)
        val id = nextId.incrementAndGet()
        val reply = CompletableDeferred<Result<String>>()
        pending[id] = reply
        nativeCall(request.toString(), id)
        val result = reply.await()
        // Журнал окна в logcat (метка klick-ui): команда и итог, без содержимого — в нём бывают ссылки подписок.
        result.onSuccess { Log.i(TAG, "$cmd → ${it.length} байт: ${it.take(120).replace(Regex("https?://\S+"), "<ссылка>")}") }
            .onFailure { Log.w(TAG, "$cmd → ${(it as? EngineError)?.code ?: it.message}") }
        return result.getOrThrow()
    }

    private const val TAG = "klick-ui"
}
