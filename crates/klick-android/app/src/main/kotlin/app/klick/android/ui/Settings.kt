package app.klick.android.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.Appearance
import app.klick.android.KlickModel
import app.klick.android.Texts
import app.klick.android.Toast
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** Что делать, если сервер не ответил (i18n.ts: serverDown). */
private val serverDownTexts = listOf(
    Triple("reconnect", "Ждать и переподключаться", "Три попытки за 15 секунд, потом уведомление «Сервер не отвечает». Дальше kl!ck проверяет раз в минуту и сам вернёт связь.") to "По умолчанию",
    Triple("next_working", "Переключиться на следующий рабочий", "Если сервер не ответил, kl!ck перейдёт на следующий сервер подключения, который отвечает.") to null,
    Triple("fastest", "Всегда самый быстрый", "kl!ck держится сервера с наименьшей задержкой и переходит на другой, когда тот становится быстрее.") to null,
)

private val themeNames = listOf("system" to "Системная", "light" to "Светлая", "dark" to "Тёмная", "custom" to "Своя")

/** «Настройки» — как ui/src/screens/Settings.tsx и iOS β: группы карточек и подэкраны. */
@Composable
fun SettingsScreen(m: KlickModel) {
    var sub by rememberSaveable { mutableStateOf<String?>(null) }
    BackHandler(sub != null) { sub = null }
    when (sub) {
        "down" -> ServerDownScreen(m) { sub = null }
        "theme" -> ThemeScreen(m) { sub = null }
        "log" -> LogScreen(m) { sub = null }
        "about" -> AboutScreen(m) { sub = null }
        else -> SettingsMain(m) { sub = it }
    }
}

@Composable
private fun SettingsMain(m: KlickModel, onSub: (String) -> Unit) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val dark = isSystemInDarkTheme()
    var version by remember { mutableStateOf<String?>(null) }
    var core by remember { mutableStateOf<String?>(null) }
    var logCount by remember { mutableStateOf<Int?>(null) }
    var toasts by remember { mutableStateOf(UiPrefs.toasts(ctx)) }
    var haptics by remember { mutableStateOf(UiPrefs.haptics(ctx)) }
    LaunchedEffect(Unit) {
        m.about().let { (v, c) -> version = v; core = c }
        logCount = m.log().size
    }
    val a = m.appearance
    val themeSummary = themeNames.firstOrNull { it.first == a.theme }?.second.orEmpty().let { name ->
        when (a.theme) {
            "custom" -> "$name · ${Palette.bases.firstOrNull { it.first == a.base }?.second.orEmpty()}"
            "system" -> "$name · сейчас ${if (dark) "тёмная" else "светлая"}"
            else -> name
        }
    }
    val pm = ctx.getSystemService(Context.POWER_SERVICE) as PowerManager
    val unrestricted = pm.isIgnoringBatteryOptimizations(ctx.packageName)

    KScreen {
        KTitle("Настройки")

        KSectionHead("Подключение")
        KCardList {
            KRow("Если сервер недоступен", serverDownTexts.firstOrNull { it.first.first == m.onServerDown }?.first?.second, chevron = true, onClick = { onSub("down") })
            KDivider()
            KRow("Постоянный VPN", "Android сам включит kl!ck после перезагрузки, а «Блокировать без VPN» не выпустит трафик мимо", wrap = true, chevron = true, onClick = {
                ctx.startActivity(Intent(Settings.ACTION_VPN_SETTINGS))
            })
            KDivider()
            KRow(
                "Работа в фоне", if (unrestricted) "Без ограничений — Android не выключит VPN ради батареи" else "Android может выключить VPN ради батареи — разрешите работу в фоне",
                wrap = true, chevron = !unrestricted,
                onClick = if (unrestricted) null else ({
                    runCatching { ctx.startActivity(Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:${ctx.packageName}"))) }
                }),
            )
        }

        KSectionHead("Уведомления")
        KCardList {
            KRow("Плашки внизу экрана", "«Связь восстановлена», «Подписка обновлена» и другие. Ошибки видно всегда", wrap = true) {
                KToggle(toasts) { toasts = it; UiPrefs.setToasts(ctx, it) }
            }
            KDivider()
            KRow("Вибрация", "Отклик на нажатия и итог подключения", wrap = true) {
                KToggle(haptics) { haptics = it; UiPrefs.setHaptics(ctx, it) }
            }
        }

        KSectionHead("Оформление")
        KCardList {
            KRow("Тема", themeSummary, chevron = true, onClick = { onSub("theme") })
            KDivider()
            KRow("Серверы по странам", "Сначала страны, внутри — их серверы", wrap = true) {
                KToggle(m.serversByCountry) { m.setPrefs(JSONObject().put("servers_by_country", it)) }
            }
        }

        KSectionHead("Общие")
        KCardList {
            KRow("Обновлять подписки", "Как просит панель, иначе раз в 12 часов", wrap = true) {
                KToggle(m.autoUpdate) { m.setPrefs(JSONObject().put("auto_update", it)) }
            }
        }

        KSectionHead("Диагностика")
        KCardList {
            KRow("Журнал", logCount?.let { Texts.plural(it, "запись", "записи", "записей") } ?: "Записи службы", chevron = true, onClick = { onSub("log") })
            KDivider()
            KRow("Скопировать отчёт", "Версии, режим и журнал — без ссылок и адресов сайтов", wrap = true, onClick = {
                scope.launch {
                    val text = m.report()
                    if (text != null) {
                        (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("kl!ck", text))
                        m.toast = Toast("Отчёт скопирован")
                    } else {
                        m.toast = Toast("Служба не отдала отчёт", tone = "bad")
                    }
                }
            }) { KIcon(KIcons.copy, 16.dp, K.dim2) }
            KDivider()
            KRow("Ядро Mihomo", core ?: "встроено в приложение") { Box(Modifier.size(8.dp).background(K.accent, CircleShape)) }
        }

        KCardList(top = 22.dp) {
            KRow("О приложении", "Версия ${version ?: "…"}", chevron = true, onClick = { onSub("about") })
        }
    }
}

/** `.back`: капсула 32 «‹ Назад» над подэкраном. */
@Composable
fun BackButton(label: String, onBack: () -> Unit) {
    Row(
        Modifier.padding(top = 14.dp).height(32.dp).clip(CircleShape).background(K.card).press(onClick = onBack).padding(start = 8.dp, end = 14.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        KIcon(KIcons.chevron, 14.dp, K.text2, Modifier.rotate(180f))
        KText(label, size = 13.sp, weight = FontWeight.Medium, color = K.text2)
    }
}

/** `.choice`: карточка выбора с кружком, названием и пояснением; выбранная — с обводкой. */
@Composable
fun ChoiceCard(title: String, desc: String?, hint: String?, on: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard().border(1.dp, if (on) K.ring else Color.Transparent, RoundedCornerShape(K.r(20)))
            .clickable(onClick = onClick).padding(14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Box(Modifier.padding(top = 1.dp)) { KRadio(on) }
        Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
            KText(title, size = 15.sp, weight = FontWeight.SemiBold)
            if (desc != null) KText(desc, size = 12.sp, color = K.text3, lineHeight = 16.sp)
            if (hint != null) KText(hint, size = 12.sp, color = K.dim2)
        }
    }
}

@Composable
private fun ServerDownScreen(m: KlickModel, onBack: () -> Unit) {
    KScreen {
        BackButton("Настройки", onBack)
        KTitle("Если сервер недоступен")
        Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            serverDownTexts.forEach { (t, hint) ->
                ChoiceCard(t.second, t.third, hint, m.onServerDown == t.first) {
                    if (m.onServerDown != t.first) m.setPrefs(JSONObject().put("on_server_down", t.first))
                }
            }
        }
    }
}

/** «Оформление»: тема карточками с превью, для своей темы — основа и акцент. */
@Composable
private fun ThemeScreen(m: KlickModel, onBack: () -> Unit) {
    val a = m.appearance
    val desc = mapOf(
        "system" to "Повторяет тему Android и переключается вместе с ней.",
        "light" to "Светлый фон и тёмный текст — удобно при ярком освещении.",
        "dark" to "Тёмный фон — меньше нагрузки на глаза вечером.",
        "custom" to "Выберите основу и цвет акцента ниже.",
    )
    KScreen {
        BackButton("Настройки", onBack)
        KText("Оформление", size = 24.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 12.dp))
        Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            themeNames.chunked(2).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    row.forEach { (k, label) -> ThemeCard(k, label, a, Modifier.weight(1f)) { m.applyAppearance(a.copy(theme = k)) } }
                }
            }
        }
        KText(desc[a.theme].orEmpty(), size = 12.sp, color = K.dim, modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 10.dp))
        if (a.theme == "custom") {
            KSectionHead("Основа")
            Box(Modifier.padding(top = 10.dp)) { KSeg(a.base, Palette.bases) { m.applyAppearance(a.copy(base = it)) } }
            KSectionHead("Акцент")
            Row(
                Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard().padding(horizontal = 16.dp, vertical = 14.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Palette.accents.forEach { c ->
                    val on = a.accent.lowercase() == Palette.hex(c)
                    Box(
                        Modifier.size(40.dp).background(if (on) rgb(c) else Color.Transparent, CircleShape).padding(2.dp).background(K.card, CircleShape).padding(2.dp)
                            .background(rgb(c), CircleShape).press { m.applyAppearance(a.copy(accent = Palette.hex(c))) },
                    )
                }
            }
            KText("Акцентом подсвечиваются кнопка подключения, переключатели и статус «подключено».", size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 12.dp))
        }
    }
}

/** `.theme-card`: превью темы (полоса с точкой акцента и карточка) и название с кружком выбора. */
@Composable
private fun ThemeCard(kind: String, label: String, a: Appearance, modifier: Modifier, onClick: () -> Unit) {
    val p = Palette.palettes.getValue(if (kind == "light") "light" else if (kind == "custom") (if (Palette.palettes.containsKey(a.base)) a.base else "graphite") else "graphite")
    val accent = rgb(if (kind == "custom") Palette.parseHex(a.accent) else if (kind == "light") 0x1fa34a else 0x30d158)
    val on = a.theme == kind
    val sys = kind == "system"
    Column(
        modifier.clip(RoundedCornerShape(K.r(20))).kCard().border(1.dp, if (on) K.ring else Color.Transparent, RoundedCornerShape(K.r(20)))
            .clickable(onClick = onClick).padding(8.dp),
    ) {
        Column(
            Modifier.fillMaxWidth().height(72.dp).clip(RoundedCornerShape(K.r(12)))
                .background(if (sys) Brush.linearGradient(0.5f to rgb(0xf5f5f7), 0.5f to rgb(0x1a1a1d)) else Brush.linearGradient(listOf(rgb(p.getValue("win")), rgb(p.getValue("win")))))
                .border(1.dp, if (sys) Color.Gray.copy(alpha = 0.3f) else rgb(p.getValue("line")), RoundedCornerShape(K.r(12))).padding(8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Box(Modifier.size(16.dp).background(accent, CircleShape))
                Box(Modifier.size(40.dp, 4.dp).background(if (sys) Color.Gray.copy(alpha = 0.6f) else rgb(p.getValue("dim")), RoundedCornerShape(2.dp)))
            }
            Box(Modifier.fillMaxWidth().weight(1f).background(if (sys) Color.Gray.copy(alpha = 0.28f) else rgb(p.getValue("card")), RoundedCornerShape(8.dp)))
        }
        Row(Modifier.padding(start = 4.dp, end = 4.dp, top = 12.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            KRadio(on)
            KText(label, weight = FontWeight.Medium)
        }
    }
}

@Composable
private fun LogScreen(m: KlickModel, onBack: () -> Unit) {
    var lines by remember { mutableStateOf<List<KlickModel.LogLine>?>(null) }
    LaunchedEffect(Unit) { lines = m.log() }
    val fmt = remember { SimpleDateFormat("HH:mm:ss", Locale.US) }
    KScreen {
        BackButton("Настройки", onBack)
        KTitle("Журнал")
        val l = lines
        when {
            l == null -> Box(Modifier.fillMaxWidth().padding(top = 40.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = K.dim, strokeWidth = 2.dp) }
            l.isEmpty() -> Box(Modifier.padding(top = 16.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).padding(22.dp), contentAlignment = Alignment.Center) {
                KText("Записей пока нет", size = 13.sp, color = K.dim)
            }
            else -> Column(Modifier.padding(top = 16.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard()) {
                l.reversed().forEachIndexed { i, line ->
                    if (i > 0) KDivider()
                    Row(Modifier.padding(horizontal = 14.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        KText(fmt.format(Date(line.at.toLong())), size = 11.sp, color = K.dim2, mono = true)
                        KText(line.text, size = 12.sp, color = when (line.level) { "error" -> K.red; "warn" -> K.orange; else -> K.text2 })
                    }
                }
            }
        }
    }
}

@Composable
private fun AboutScreen(m: KlickModel, onBack: () -> Unit) {
    val ctx = LocalContext.current
    var version by remember { mutableStateOf<String?>(null) }
    var core by remember { mutableStateOf<String?>(null) }
    LaunchedEffect(Unit) { m.about().let { (v, c) -> version = v; core = c } }
    val authors = listOf(
        Triple("vbu00", "Разработка", "https://github.com/vbu00"),
        Triple("Dmitriy Medvedev", "Дизайн интерфейса и логотип", "https://github.com/aleuuu"),
        Triple("limeflash", "Порты на macOS и iOS", "https://github.com/limeflash"),
    )
    fun open(url: String) = runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
    KScreen {
        BackButton("Настройки", onBack)
        Column(Modifier.fillMaxWidth().padding(top = 18.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.size(72.dp).clip(RoundedCornerShape(K.r(20))).kCard(), contentAlignment = Alignment.Center) { KIcon(KIcons.power, 32.dp, K.accent) }
            KText("kl!ck", size = 22.sp, weight = FontWeight.Bold, modifier = Modifier.padding(top = 12.dp))
            KText("Версия ${version ?: "…"} · Android", size = 12.sp, color = K.dim, modifier = Modifier.padding(top = 4.dp))
        }
        KCardList(top = 24.dp) {
            Kv("Ядро", "Mihomo ${core ?: "встроено"}")
            KDivider()
            Kv("Система", "Android ${Build.VERSION.RELEASE} · ${Build.MANUFACTURER} ${Build.MODEL}")
        }
        KSectionHead("Авторы")
        KCardList {
            authors.forEachIndexed { i, (name, role, url) ->
                if (i > 0) KDivider()
                KRow(name, role, chevron = true, onClick = { open(url) })
            }
        }
        KCardList(top = 22.dp) { KRow("Исходный код", "github.com/vbu00/klick", chevron = true, onClick = { open("https://github.com/vbu00/klick") }) }
        Spacer(Modifier.width(1.dp))
    }
}

@Composable
private fun Kv(k: String, v: String) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 12.dp)) {
        KText(k, size = 13.sp, color = K.dim)
        Spacer(Modifier.weight(1f))
        KText(v, size = 13.sp, lines = 1)
    }
}

/** Настройки окна на этом телефоне (не в движке): плашки и вибрация. Состояние Compose — меняются сразу. */
object UiPrefs {
    var toastsOn by mutableStateOf(true)
        private set
    var hapticsOn by mutableStateOf(true)
        private set

    private fun p(ctx: Context) = ctx.getSharedPreferences("klick-ui", Context.MODE_PRIVATE)

    fun init(ctx: Context) {
        toastsOn = p(ctx).getBoolean("toasts", true)
        hapticsOn = p(ctx).getBoolean("haptics", true)
    }

    fun toasts(@Suppress("UNUSED_PARAMETER") ctx: Context) = toastsOn
    fun haptics(@Suppress("UNUSED_PARAMETER") ctx: Context) = hapticsOn

    fun setToasts(ctx: Context, on: Boolean) {
        toastsOn = on
        p(ctx).edit().putBoolean("toasts", on).apply()
    }

    fun setHaptics(ctx: Context, on: Boolean) {
        hapticsOn = on
        p(ctx).edit().putBoolean("haptics", on).apply()
    }
}
