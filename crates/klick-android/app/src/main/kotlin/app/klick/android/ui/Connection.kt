package app.klick.android.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.rememberModalBottomSheetState
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
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.Failure
import app.klick.android.IpColumn
import app.klick.android.IpReport
import app.klick.android.KlickModel
import app.klick.android.LiveConn
import app.klick.android.Rule
import app.klick.android.Service
import app.klick.android.Target
import app.klick.android.Texts
import app.klick.android.Toast
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * «Соединение» — как ui/src/screens/Connection.tsx и iOS β: куда направлять с готовыми наборами, мой список,
 * «Сейчас в сети», «Не открывается?», живая схема и «Как вас видят сайты». Везде один тумблер — «через VPN».
 */
@Composable
fun ConnectionScreen(m: KlickModel) {
    var sub by rememberSaveable { mutableStateOf<String?>(null) }
    var adding by remember { mutableStateOf(false) }
    var conns by remember { mutableStateOf<List<LiveConn>>(emptyList()) }
    var fails by remember { mutableStateOf<List<Failure>>(emptyList()) }
    BackHandler(sub != null) { sub = null }
    LaunchedEffect(Unit) { m.loadCatalog() }
    // Соединения и неудачи раз в 2 секунды, пока открыта вкладка и работает VPN.
    LaunchedEffect(m.isRunning) {
        if (!m.isRunning) {
            conns = emptyList()
            fails = emptyList()
            return@LaunchedEffect
        }
        while (true) {
            m.connections()?.let { conns = it }
            m.failures()?.let { fails = it }
            delay(2000)
        }
    }
    val sites = Live.sites(conns, m.list, m.catalog)
    when (sub) {
        "list" -> MyList(m, onBack = { sub = null }, onAdd = { adding = true })
        "live" -> LiveScreen(m, sites) { sub = null }
        "failures" -> FailuresScreen(m, fails) { sub = null }
        else -> ConnectionMain(m, sites, conns, fails, onSub = { sub = it }, onAdd = { adding = true })
    }
    if (adding) AddRuleSheet(m) { adding = false }
}

@Composable
private fun ConnectionMain(m: KlickModel, sites: List<SiteRow>, conns: List<LiveConn>, fails: List<Failure>, onSub: (String) -> Unit, onAdd: () -> Unit) {
    val routing = m.routing
    val list = m.list
    KScreen {
        KTitle("Соединение", xl = true)

        CxHead("Куда направлять")
        KSeg(routing, listOf("all_vpn" to "VPN для всего", "selected" to "VPN для выбранного"), height = 38.dp, size = 14.sp) { m.setRouting(it) }
        Column(Modifier.padding(top = 8.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
            if (routing == "selected") {
                Preset("Заблокированное в РФ — через VPN", "Готовый набор, обновляется сам. Остальное — напрямую.", m.blockedPreset) {
                    m.setPrefs(JSONObject().put("blocked_preset", it))
                }
            } else {
                Preset(".ru и .рф напрямую", "Госуслуги, банки, Яндекс — без VPN", m.ruDomains) { m.setPrefs(JSONObject().put("ru_domains", it)) }
                CxLine()
                Preset("Российские IP напрямую", "По базе стран — кроме заблокированного в РФ: оно всё равно через VPN", m.ruIps) {
                    m.setPrefs(JSONObject().put("ru_ips", it))
                }
                CxLine()
                Preset("Локальная сеть — напрямую", "Роутер, принтер, игры по локалке", null) {}
            }
        }

        CxHead("Мой список", wide = true)
        ListCard(m, routing, list) { onSub("list") }
        CardButton("Добавить", KIcons.plus, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp), onClick = onAdd)

        CxHead("Сейчас в сети", wide = true)
        Box(Modifier.clickable { onSub("live") }) { LiveSummary(sites, m.isRunning) }
        if (fails.isNotEmpty()) {
            Row(
                Modifier.padding(top = 8.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).clickable { onSub("failures") }
                    .padding(horizontal = 16.dp, vertical = 13.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    KText("Не открывается?", weight = FontWeight.SemiBold)
                    KText("Недавние неудачные соединения", size = 12.5.sp, color = K.dim)
                }
                KText("${fails.size}", size = 13.sp, mono = true, color = K.orange)
                KIcon(KIcons.chevron, 14.dp, K.dim)
            }
        }

        CxHead("Схема · сейчас", wide = true)
        LiveSchema(m.isRunning, m.state?.server, conns, m.catalog)

        IpCard(m)
    }
}

@Composable
private fun ListCard(m: KlickModel, routing: String, list: List<Rule>, onClick: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Column(Modifier.weight(1f)) {
            KText("${if (routing == "selected") "Через VPN" else "Напрямую"} · мой список", size = 16.sp, weight = FontWeight.SemiBold)
            KText(if (list.isEmpty()) "Пока пусто — добавьте сервис или сайт" else Rules.summary(list), size = 12.5.sp, color = K.dim, modifier = Modifier.padding(top = 3.dp))
            if (list.isNotEmpty()) {
                Row(Modifier.padding(top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                    list.take(6).forEachIndexed { i, r ->
                        Box(Modifier.offset(x = (-6 * i).dp).alpha(if (r.enabled) 1f else 0.4f).border(2.dp, K.card, RoundedCornerShape(8.dp))) {
                            RuleTile(Rules.title(r.target, m.catalog), 28)
                        }
                    }
                    KText(
                        "включено ${list.count { it.enabled }} из ${list.size}", size = 12.sp, color = K.dim,
                        modifier = Modifier.offset(x = (-6 * (list.take(6).size - 1)).dp).padding(start = 12.dp),
                    )
                }
            }
        }
        KIcon(KIcons.chevron, 14.dp, K.dim)
    }
}

// ── Общее ────────────────────────────────────────────────────

/** `.cx-head`: подпись раздела «Соединения» — 12, полужирный, заглавными. */
@Composable
private fun CxHead(text: String, wide: Boolean = false, small: Boolean = false) {
    KText(
        text.uppercase(), size = 12.sp, weight = FontWeight.SemiBold, tracking = 0.72.sp, color = K.dim,
        modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = if (small) 18.dp else if (wide) 22.dp else 20.dp, bottom = 8.dp),
    )
}

@Composable
private fun CxLine() {
    Box(Modifier.fillMaxWidth().height(1.dp).background(K.line))
}

/** Готовый набор: название, пояснение и тумблер (или «всегда»). */
@Composable
private fun Preset(title: String, desc: String, on: Boolean?, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
            KText(title, weight = FontWeight.SemiBold)
            KText(desc, size = 12.sp, color = K.dim, lineHeight = 16.sp)
        }
        if (on != null) KToggle(on, onChange) else KText("всегда", size = 12.sp, color = K.dim2)
    }
}

/** Подзаголовок подэкрана (`.sub-title`, `.sub-lead`). */
@Composable
private fun SubHead(title: String, lead: String? = null) {
    Column(Modifier.padding(top = 12.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        KText(title, size = 24.sp, weight = FontWeight.SemiBold)
        if (lead != null) KText(lead, size = 13.sp, color = K.dim, lineHeight = 18.sp)
    }
}

@Composable
private fun CxEmpty(text: String) {
    Box(Modifier.padding(top = 14.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).padding(22.dp), contentAlignment = Alignment.Center) {
        KText(text, size = 13.sp, color = K.dim, center = true, lineHeight = 18.sp)
    }
}

@Composable
private fun FootNote(text: String) {
    KText(text, size = 12.sp, color = K.dim2, lineHeight = 17.sp, modifier = Modifier.padding(top = 12.dp))
}

/** Строка правила или сайта: плитка, название, подпись и тумблер. */
@Composable
private fun RuleRow(label: String, title: String, sub: String, subVpn: Boolean = false, dimmed: Boolean = false, trailing: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().padding(start = 14.dp, end = 10.dp, top = 11.dp, bottom = 11.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.alpha(if (dimmed) 0.45f else 1f)) { RuleTile(label) }
        Column(Modifier.weight(1f)) {
            KText(title, weight = FontWeight.SemiBold, color = if (dimmed) K.dim else K.text, lines = 1)
            KText(sub, size = 11.5.sp, color = if (subVpn) K.accent else K.dim, lines = 1)
        }
        trailing()
    }
}

// ── Правила ──────────────────────────────────────────────────

/** Названия, цвета и сводки правил — как ui/src/lib/rules.ts. */
object Rules {
    private val tints = listOf(0xa8c7faL, 0xf5c26bL, 0xc4b5fdL, 0x9ad9c0L, 0xf4a7b9L, 0xb8c4d0L)

    fun tint(key: String): Color {
        var h = 0u
        for (ch in key) h = h * 31u + ch.code.toUInt()
        return rgb(tints[(h % tints.size.toUInt()).toInt()])
    }

    fun title(t: Target, catalog: List<Service>): String = when (t.kind) {
        "service" -> catalog.firstOrNull { it.id == t.value }?.name ?: t.value
        "domain" -> if ("." in t.value) t.value else ".${t.value}"
        "ip" -> t.value.removeSuffix("/32").removeSuffix("/128")
        else -> t.value
    }

    /** «3 сервиса · 2 сайта». */
    fun summary(list: List<Rule>): String {
        val services = list.count { it.target.kind == "service" }
        val sites = list.count { it.target.kind == "domain" || it.target.kind == "ip" }
        return listOfNotNull(
            if (services > 0) Texts.plural(services, "сервис", "сервиса", "сервисов") else null,
            if (sites > 0) Texts.plural(sites, "сайт", "сайта", "сайтов") else null,
        ).joinToString(" · ")
    }

    fun routeName(r: String) = when (r) {
        "vpn" -> "через VPN"
        "block" -> "блок"
        else -> "напрямую"
    }
}

/** `.tile-letter`: первая буква на цветной плитке. */
@Composable
fun RuleTile(label: String, size: Int = 30) {
    Box(Modifier.size(size.dp).background(Rules.tint(label), RoundedCornerShape(8.dp)), contentAlignment = Alignment.Center) {
        KText(label.trim().take(1).uppercase(), size = if (size < 30) 11.sp else 12.sp, weight = FontWeight.Bold, color = rgb(0x1a1a1d))
    }
}

// ── Мой список ───────────────────────────────────────────────

@Composable
private fun MyList(m: KlickModel, onBack: () -> Unit, onAdd: () -> Unit) {
    var tab by rememberSaveable { mutableStateOf("all") }
    val routing = m.routing
    val list = m.list
    val enabled = list.count { it.enabled }
    fun kind(r: Rule) = if (r.target.kind == "domain" || r.target.kind == "ip") "site" else r.target.kind
    val groups = listOf("service" to "Сервисы", "site" to "Сайты и IP")
        .filter { tab == "all" || tab == it.first }
        .map { (k, label) -> Triple(k, label, list.withIndex().filter { kind(it.value) == k }) }
        .filter { it.third.isNotEmpty() }
    KScreen {
        BackButton("Соединение", onBack)
        SubHead(
            "${if (routing == "selected") "Через VPN" else "Напрямую"} · мой список",
            "Положение «${if (routing == "all_vpn") "VPN для всего" else "VPN для выбранного"}». Включено $enabled из ${list.size}. Выключенное правило не работает, но остаётся в списке.",
        )
        Box(Modifier.padding(top = 14.dp)) { KSeg(tab, listOf("all" to "Все", "service" to "Сервисы", "site" to "Сайты"), height = 32.dp, size = 12.5.sp) { tab = it } }
        groups.forEach { (_, label, rows) ->
            CxHead(label, small = true)
            Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
                rows.forEachIndexed { i, (index, r) ->
                    if (i > 0) CxLine()
                    RuleLine(m, routing, index, r)
                }
            }
        }
        if (groups.isEmpty()) {
            CxEmpty(if (routing == "selected") "Добавьте сервис или сайт — они пойдут через VPN." else "Добавьте то, что должно идти напрямую: банк, локальный сервис.")
        }
        CardButton("Добавить", KIcons.plus, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 12.dp), onClick = onAdd)
    }
}

@Composable
private fun RuleLine(m: KlickModel, routing: String, index: Int, r: Rule) {
    val title = Rules.title(r.target, m.catalog)
    val exception = (r.route ?: "") != (if (routing == "selected") "vpn" else "direct")
    Row(Modifier.fillMaxWidth().padding(start = 14.dp, end = 10.dp, top = 11.dp, bottom = 11.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.alpha(if (r.enabled) 1f else 0.45f)) { RuleTile(title) }
        KText(title, weight = FontWeight.SemiBold, color = if (r.enabled) K.text else K.dim, lines = 1, modifier = Modifier.weight(1f))
        val route = r.route
        if (exception && route != null) {
            val tone = when (route) { "vpn" -> K.accent; "block" -> K.red; else -> K.text2 }
            Box(
                Modifier.height(22.dp).clip(CircleShape).background(if (route == "direct") K.elem else tone.copy(alpha = 0.16f)).padding(horizontal = 8.dp),
                contentAlignment = Alignment.Center,
            ) { KText(Rules.routeName(route), size = 11.sp, weight = FontWeight.SemiBold, color = tone) }
        }
        KToggle(r.enabled) { m.setRuleEnabled(routing, index, it) }
        Box(Modifier.size(26.dp).clip(CircleShape).clickable { m.removeRule(routing, index) }, contentAlignment = Alignment.Center) {
            KText("✕", size = 12.sp, color = K.dim2)
        }
    }
}

// ── Добавить в список ────────────────────────────────────────

/**
 * Лист «Добавить в список»: сервисы каталога или сайт и IP. Правила встают в список текущего положения: в
 * «VPN для выбранного» — через VPN, в «VPN для всего» — напрямую.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun AddRuleSheet(m: KlickModel, onClose: () -> Unit) {
    val position = m.routing
    val route = if (position == "selected") "vpn" else "direct"
    var tab by remember { mutableStateOf("services") }
    var picked by remember { mutableStateOf<List<String>>(emptyList()) }
    var site by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val list = m.list
    val free = m.catalog.filter { s -> list.none { it.target.kind == "service" && it.target.value.equals(s.id, ignoreCase = true) } }
    val siteItems = site.split(' ', ',', ';', '\n', '\t').filter { it.isNotBlank() }
    val count = picked.size + siteItems.size

    fun submit() {
        if (count == 0 || busy) return
        busy = true
        val rules = picked.map { Target("service", it) to route } + siteItems.map { v ->
            val ip = Regex("^[\\d.]+(/\\d+)?$").matches(v) || ':' in v
            Target(if (ip) "ip" else "domain", v) to route
        }
        scope.launch {
            val added = m.addRules(position, rules)
            busy = false
            if (added > 0) {
                m.toast = Toast(if (added == 1) "Добавлено в список" else "Добавлено правил: $added", "Пойдёт ${Rules.routeName(route)}")
                onClose()
            }
        }
    }

    ModalBottomSheet(onDismissRequest = onClose, sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true), containerColor = K.win, dragHandle = null) {
        Column(Modifier.fillMaxWidth().padding(18.dp)) {
            KText("Добавить в «${if (position == "selected") "Через VPN" else "Напрямую"}»", size = 20.sp, weight = FontWeight.Bold)
            KText(
                if (position == "selected") "Выбранное пойдёт через VPN, остальное — напрямую." else "Выбранное пойдёт мимо VPN: банк, игра, локальный сервис.",
                size = 12.5.sp, color = K.dim, modifier = Modifier.padding(top = 4.dp),
            )
            Box(Modifier.padding(top = 14.dp)) { KSeg(tab, listOf("services" to "Сервисы", "site" to "Сайт или IP")) { tab = it } }
            Column(Modifier.padding(vertical = 12.dp).heightIn(max = 420.dp).verticalScroll(rememberScrollState())) {
                if (tab == "services") {
                    free.chunked(2).forEach { row ->
                        Row(Modifier.padding(bottom = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            row.forEach { s ->
                                val on = s.id in picked
                                Row(
                                    Modifier.weight(1f).clip(RoundedCornerShape(K.r(12))).kCard(12).border(1.dp, if (on) K.accent else Color.Transparent, RoundedCornerShape(K.r(12)))
                                        .clickable { picked = if (on) picked - s.id else picked + s.id }.padding(10.dp),
                                    verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
                                ) {
                                    RuleTile(s.name, 26)
                                    KText(s.name, size = 13.sp, weight = FontWeight.SemiBold, lines = 1, modifier = Modifier.weight(1f))
                                    Check(on)
                                }
                            }
                            if (row.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                    if (free.isEmpty()) KText(if (m.catalog.isEmpty()) "Загружаю каталог…" else "Все сервисы каталога уже в списке", size = 13.sp, color = K.dim, modifier = Modifier.padding(18.dp))
                } else {
                    Box(Modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(K.r(12))).kCard(12).border(1.dp, K.line, RoundedCornerShape(K.r(12))).padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
                        if (site.isEmpty()) KText("example.com или 1.2.3.4", color = K.dim2, mono = true)
                        BasicTextField(
                            site, { site = it }, Modifier.fillMaxWidth(), singleLine = true,
                            textStyle = TextStyle(fontSize = 14.sp, color = K.text, fontFamily = FontFamily.Monospace), cursorBrush = SolidColor(K.accent),
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done, autoCorrectEnabled = false),
                            keyboardActions = KeyboardActions(onDone = { submit() }),
                        )
                    }
                    KText("Поддомены включаются автоматически.", size = 12.sp, color = K.dim, modifier = Modifier.padding(start = 4.dp, top = 8.dp))
                }
            }
            Box(
                Modifier.fillMaxWidth().height(46.dp).clip(RoundedCornerShape(K.r(12))).background(if (count > 0) K.accent else K.card)
                    .press(count > 0 && !busy) { submit() },
                contentAlignment = Alignment.Center,
            ) { KText(if (busy) "Добавляю…" else if (count > 0) "Добавить · $count" else "Добавить", weight = FontWeight.SemiBold, color = if (count > 0) K.onAccent else K.dim2) }
        }
    }
}

/** `.ax-check`: квадратик выбора. */
@Composable
private fun Check(on: Boolean) {
    val accent = K.accent
    val ring = K.ring
    val onAccent = K.onAccent
    Canvas(Modifier.size(18.dp)) {
        val r = androidx.compose.ui.geometry.CornerRadius(5.dp.toPx())
        if (on) drawRoundRect(accent, cornerRadius = r) else drawRoundRect(ring, cornerRadius = r, style = Stroke(1.5.dp.toPx()))
        if (on) {
            val p = Path().apply {
                moveTo(size.width * 0.25f, size.height * 0.52f)
                lineTo(size.width * 0.40f, size.height * 0.67f)
                lineTo(size.width * 0.70f, size.height * 0.34f)
            }
            drawPath(p, onAccent, style = Stroke(2.2.dp.toPx(), cap = StrokeCap.Round))
        }
    }
}

// ── Сейчас в сети ────────────────────────────────────────────

/** Сайт в «Сейчас в сети»: хост, сколько соединений и байт, куда идёт и какое правило запишет тумблер. */
data class SiteRow(val host: String, val conns: Int, val bytes: Double, val vpn: Boolean, val target: Target)

/** Разбор соединений — как ui/src/lib/live.ts, только сайты: Android не говорит ядру, чьё соединение. */
object Live {
    fun isIp(h: String) = Regex("^[\\d.]+$").matches(h) || ':' in h

    private fun hostMatches(host: String, domain: String): Boolean {
        val h = host.lowercase()
        val d = domain.lowercase().removePrefix(".")
        return h == d || h.endsWith(".$d")
    }

    private fun ipInCidr(ip: String, cidr: String): Boolean {
        val net = cidr.substringBefore('/')
        fun num(s: String): Long? = s.split('.').mapNotNull { it.toLongOrNull() }.takeIf { it.size == 4 }?.fold(0L) { a, b -> (a shl 8) or (b and 255) }
        val a = num(ip)
        val b = num(net)
        if (a == null || b == null) return ip.equals(net, ignoreCase = true)
        val bits = cidr.substringAfter('/', "32").toIntOrNull() ?: 32
        val mask = if (bits == 0) 0L else (0xffffffffL shl (32 - bits)) and 0xffffffffL
        return a and mask == b and mask
    }

    fun service(host: String, catalog: List<Service>): Service? {
        val h = host.lowercase()
        return catalog.firstOrNull { s -> s.domains.any { h == it || h.endsWith(".$it") } }
    }

    /** Сайт без поддоменов: две последние части, три — у `co.uk`, `com.ru` и похожих. */
    fun baseDomain(host: String): String {
        val labels = host.lowercase().split('.').filter { it.isNotEmpty() }
        if (labels.size <= 2) return labels.joinToString(".")
        val sld = labels[labels.size - 2]
        val tld = labels.last()
        val three = tld.length == 2 && sld in setOf("co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go", "msk", "spb")
        return labels.takeLast(if (three) 3 else 2).joinToString(".")
    }

    /** Сайт для правила: `rr3.googlevideo.com` → сервис YouTube; `cdn.example.co.uk` → `example.co.uk`. */
    fun target(host: String, catalog: List<Service>): Target {
        service(host, catalog)?.let { return Target("service", it.id) }
        if (isIp(host)) return Target("ip", host)
        return Target("domain", baseDomain(host))
    }

    /** Включённое правило сайта, сервиса или IP для хоста: точное раньше общего, своё раньше сервиса. */
    fun rule(list: List<Rule>, host: String, catalog: List<Service>): Rule? {
        var best: Pair<Rule, Int>? = null
        for (r in list) {
            if (!r.enabled) continue
            val t = r.target
            var rank = -1
            if (t.kind == "domain" && hostMatches(host, t.value)) {
                rank = t.value.split('.').size * 2 + 1
            } else if (t.kind == "ip" && isIp(host) && ipInCidr(host, t.value)) {
                rank = 100 + (t.value.substringAfter('/', "32").toIntOrNull() ?: 32) * 2 + 1
            } else if (t.kind == "service") {
                val svc = catalog.firstOrNull { it.id == t.value }
                if (svc != null) {
                    val d = svc.domains.filter { hostMatches(host, it) }.maxByOrNull { it.split('.').size }
                    if (d != null) rank = d.split('.').size * 2 else if (isIp(host) && svc.cidrs.any { ipInCidr(host, it) }) rank = 100
                }
            }
            if (rank >= 0 && (best == null || rank > best.second)) best = r to rank
        }
        return best?.first
    }

    /** Сайты сейчас: сначала те, что качают больше. Куда идёт — по правилу из списка, иначе как решило ядро. */
    fun sites(conns: List<LiveConn>, list: List<Rule>, catalog: List<Service>): List<SiteRow> {
        val map = LinkedHashMap<String, SiteRow>()
        for (c in conns) {
            if (c.route == "block") continue
            val raw = c.host.removePrefix("www.")
            val host = raw.ifEmpty { "—" }
            val bytes = c.download + c.upload
            val s = map[host]
            map[host] = if (s != null) s.copy(conns = s.conns + 1, bytes = s.bytes + bytes)
            else SiteRow(host, 1, bytes, rule(list, c.host, catalog)?.let { it.route == "vpn" } ?: (c.route == "vpn"), target(c.host, catalog))
        }
        return map.values.sortedByDescending { it.bytes }
    }
}

/** Сводка «Сейчас в сети»: сколько соединений, доля VPN и самые активные сайты. */
@Composable
private fun LiveSummary(sites: List<SiteRow>, running: Boolean) {
    val total = sites.sumOf { it.conns }
    val vpn = sites.filter { it.vpn }.sumOf { it.conns }
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
        Column(Modifier.fillMaxWidth().padding(start = 14.dp, end = 14.dp, top = 14.dp, bottom = 12.dp)) {
            KText(Texts.plural(total, "соединение", "соединения", "соединений"), size = 15.sp, weight = FontWeight.SemiBold)
            Row(Modifier.padding(top = 10.dp).fillMaxWidth().height(8.dp).clip(RoundedCornerShape(4.dp)).background(K.elem), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                if (vpn > 0) Box(Modifier.weight(vpn.toFloat()).height(8.dp).background(K.accent))
                if (total > vpn) Box(Modifier.weight((total - vpn).toFloat()).height(8.dp).background(K.text2))
            }
            Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                Legend(K.accent, "VPN", vpn)
                Legend(K.text2, "Напрямую", total - vpn)
            }
            if (!running) KText("Соединения появятся после подключения", size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 8.dp))
        }
        sites.take(3).forEach { s ->
            CxLine()
            Row(Modifier.fillMaxWidth().padding(horizontal = 14.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                RuleTile(s.host, 28)
                Column(Modifier.weight(1f)) {
                    KText(s.host, size = 13.5.sp, weight = FontWeight.SemiBold, lines = 1)
                    KText("${s.conns} соед. · ${"%.1f".format(Locale("ru"), s.bytes / 1e6)} МБ", size = 11.5.sp, color = K.dim)
                }
                Box(Modifier.size(52.dp, 5.dp).background(if (s.vpn) K.accent else K.text2, CircleShape))
            }
        }
    }
}

@Composable
private fun Legend(c: Color, title: String, n: Int) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(7.dp).background(c, CircleShape))
        KText(title, size = 12.sp, color = K.dim)
        KText("$n", size = 12.sp, weight = FontWeight.SemiBold)
    }
}

@Composable
private fun LiveScreen(m: KlickModel, sites: List<SiteRow>, onBack: () -> Unit) {
    var filter by remember { mutableStateOf("all") }
    var paused by remember { mutableStateOf(false) }
    var frozen by remember { mutableStateOf<List<SiteRow>>(emptyList()) }
    val shown = if (paused) frozen else sites
    val nVpn = shown.count { it.vpn }
    val rows = shown.filter { filter == "all" || (filter == "vpn") == it.vpn }
    KScreen {
        BackButton("Соединение", onBack)
        Row(Modifier.padding(top = 14.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            KText("Сейчас в сети", size = 24.sp, weight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            Row(
                Modifier.height(30.dp).clip(CircleShape).background(if (paused) K.card else K.accent.copy(alpha = 0.14f))
                    .press(m.isRunning) {
                        if (!paused) frozen = sites
                        paused = !paused
                    }.padding(horizontal = 12.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(7.dp),
            ) {
                Box(Modifier.size(7.dp).background(if (paused) K.dim else K.accent, CircleShape))
                KText(if (paused) "Пауза" else "Live", size = 12.5.sp, weight = FontWeight.SemiBold, color = if (paused) K.dim else K.accent)
            }
        }
        KText("Тумблер включён — идёт через VPN. Правило сохранится в мой список.", size = 13.sp, color = K.dim, modifier = Modifier.padding(top = 6.dp))
        Row(Modifier.padding(top = 10.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            FilterChip(filter == "all", "Все", shown.size, K.dim) { filter = "all" }
            FilterChip(filter == "vpn", "VPN", nVpn, K.accent) { filter = "vpn" }
            FilterChip(filter == "direct", "Прямо", shown.size - nVpn, K.text2) { filter = "direct" }
        }
        if (!m.isRunning) {
            CxEmpty("VPN выключен. Соединения появятся после подключения.")
        } else {
            Column(Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
                rows.forEachIndexed { i, s ->
                    if (i > 0) CxLine()
                    RuleRow(s.host, s.host, if (s.vpn) "через VPN" else "напрямую", subVpn = s.vpn) {
                        KToggle(s.vpn) { m.routeTarget(s.target, it, Rules.title(s.target, m.catalog)) }
                    }
                }
                if (rows.isEmpty()) Box(Modifier.fillMaxWidth().padding(22.dp), contentAlignment = Alignment.Center) { KText("Ничего нет", size = 13.sp, color = K.dim) }
            }
        }
    }
}

@Composable
private fun FilterChip(on: Boolean, title: String, n: Int, dot: Color, onClick: () -> Unit) {
    Row(
        Modifier.height(30.dp).clip(CircleShape).background(if (on) K.raised else Color.Transparent).border(1.dp, if (on) K.raised else K.line, CircleShape)
            .clickable(onClick = onClick).padding(horizontal = 11.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(6.dp).background(dot, CircleShape))
        KText(title, size = 12.5.sp, weight = FontWeight.SemiBold, color = if (on) K.text else K.text2)
        KText("$n", size = 12.5.sp, weight = FontWeight.Medium, color = K.dim)
    }
}

// ── Не открывается? ──────────────────────────────────────────

@Composable
private fun FailuresScreen(m: KlickModel, fails: List<Failure>, onBack: () -> Unit) {
    KScreen {
        BackButton("Соединение", onBack)
        SubHead("Не открывается?", "Недавние неудачные соединения. Если сайт не открывается напрямую, пустите его через VPN — и наоборот.")
        if (fails.isEmpty()) {
            CxEmpty("Неудачных соединений нет")
        } else {
            Column(Modifier.padding(top = 14.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
                fails.forEachIndexed { i, f ->
                    if (i > 0) CxLine()
                    val target = Live.target(f.host, m.catalog)
                    val vpn = Live.rule(m.list, f.host, m.catalog)?.let { it.route == "vpn" } ?: (f.route == "vpn")
                    RuleRow(f.host, f.host, "${Rules.routeName(f.route)} · ${reason(f.error)} · ${ago(f.at)}") {
                        KToggle(vpn) { m.routeTarget(target, it, Rules.title(target, m.catalog)) }
                    }
                }
            }
        }
        FootNote("Причина обрыва известна не всегда. Список хранится только в памяти и пропадает при выходе.")
    }
}

private fun ago(at: Double): String {
    val mins = Math.round((System.currentTimeMillis() / 1000.0 - at) / 60).toInt().coerceAtLeast(0)
    return if (mins < 1) "только что" else if (mins < 60) "$mins мин назад" else "${Math.round(mins / 60.0)} ч назад"
}

/** i18n.ts: failureReason. */
private fun reason(error: String): String {
    val e = error.lowercase()
    return when {
        "timeout" in e || "deadline" in e -> "нет ответа"
        "reset" in e -> "сброс соединения"
        "refused" in e -> "отказ сервера"
        "no such host" in e || "dns" in e -> "имя не найдено"
        "eof" in e || "closed" in e -> "обрыв"
        "unreachable" in e -> "адрес недоступен"
        else -> "ошибка соединения"
    }
}

// ── Схема ────────────────────────────────────────────────────

/** Живая схема: устройство → через VPN или напрямую → интернет; под ней — кто идёт по каждой линии. */
@Composable
private fun LiveSchema(running: Boolean, server: String?, conns: List<LiveConn>, catalog: List<Service>) {
    fun lane(route: String): Pair<List<String>, Int> {
        val bytes = LinkedHashMap<String, Double>()
        for (c in conns) {
            if (c.route != route) continue
            val name = Live.service(c.host, catalog)?.name ?: c.host.removePrefix("www.")
            bytes[name] = (bytes[name] ?: 0.0) + c.download + c.upload
        }
        val sorted = bytes.entries.sortedByDescending { it.value }.map { it.key }
        return sorted.take(3) to (sorted.size - 3).coerceAtLeast(0)
    }
    val vpn = lane("vpn")
    val direct = lane("direct")
    val block = lane("block")
    val vpnOn = running && vpn.first.isNotEmpty()
    val directOn = !running || direct.first.isNotEmpty()
    Column(Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(16))).kCard(16).padding(8.dp)) {
        BoxWithConstraints(Modifier.fillMaxWidth().height(150.dp)) {
            val w = maxWidth
            val h = maxHeight
            FlowPath(up = true, active = vpnOn, color = K.accent, speedMs = 700)
            FlowPath(up = false, active = directOn, color = K.dim, speedMs = 900)
            Node("Устройство", "приложения", Modifier.offset(x = w * 0.14f - 36.dp, y = h * 0.5f - 34.dp)) {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    Box(Modifier.size(20.dp, 14.dp).border(2.dp, K.text2, RoundedCornerShape(3.dp)))
                    Box(Modifier.size(10.dp, 2.dp).background(K.text2, RoundedCornerShape(1.dp)))
                }
            }
            SchemaChip(if (running) server ?: "VPN" else "VPN выключен", live = running, Modifier.align(Alignment.TopCenter).offset(y = h * 0.24f - 14.dp))
            PlainChip("Напрямую", Modifier.align(Alignment.TopCenter).offset(y = h * 0.76f - 14.dp))
            Node("Интернет", "сайты", Modifier.offset(x = w * 0.86f - 36.dp, y = h * 0.5f - 34.dp)) { KIcon(KIcons.globe, 20.dp, K.text2) }
        }
        Column(Modifier.fillMaxWidth().padding(start = 6.dp, end = 6.dp, top = 4.dp, bottom = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (running) {
                LaneText(K.accent, "через VPN", vpn)
                LaneText(K.dim, "напрямую", direct)
                if (block.first.isNotEmpty()) LaneText(K.red, "блок", block)
            } else {
                KText("VPN выключен: всё идёт напрямую. Схема оживёт после подключения.", size = 12.sp, color = K.dim)
            }
        }
    }
}

@Composable
private fun LaneText(c: Color, title: String, l: Pair<List<String>, Int>) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Box(Modifier.padding(top = 4.dp).size(8.dp).background(c, CircleShape))
        val names = if (l.first.isEmpty()) "пока никого" else l.first.joinToString(", ") + if (l.second > 0) " +${l.second}" else ""
        Row {
            KText(title, size = 12.sp, weight = FontWeight.SemiBold)
            KText(" — $names", size = 12.sp, color = K.text3)
        }
    }
}

@Composable
private fun Node(title: String, sub: String, modifier: Modifier, icon: @Composable () -> Unit) {
    Column(modifier.width(72.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Box(Modifier.size(40.dp).clip(RoundedCornerShape(K.r(12))).background(K.elem), contentAlignment = Alignment.Center) { icon() }
        KText(title, size = 12.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 6.dp))
        KText(sub, size = 11.sp, color = K.dim)
    }
}

@Composable
private fun SchemaChip(text: String, live: Boolean, modifier: Modifier) {
    Row(
        modifier.widthIn(max = 190.dp).height(28.dp).clip(RoundedCornerShape(K.r(9))).background(if (live) K.accent.copy(alpha = 0.16f) else K.win)
            .border(1.dp, if (live) K.accent.copy(alpha = 0.4f) else K.ring, RoundedCornerShape(K.r(9))).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Box(Modifier.size(6.dp).background(if (live) K.accent else Color.Transparent, CircleShape).border(1.5.dp, if (live) Color.Transparent else K.ring, CircleShape))
        KText(text, size = 12.sp, weight = FontWeight.SemiBold, color = if (live) K.text else K.dim, lines = 1)
    }
}

@Composable
private fun PlainChip(text: String, modifier: Modifier) {
    Box(
        modifier.height(28.dp).clip(RoundedCornerShape(K.r(9))).background(K.win).border(1.dp, K.line, RoundedCornerShape(K.r(9))).padding(horizontal = 10.dp),
        contentAlignment = Alignment.Center,
    ) { KText(text, size = 12.sp, weight = FontWeight.SemiBold) }
}

/** Линия схемы: кривая от устройства к интернету через верх (VPN) или низ (напрямую); идёт трафик — бегут штрихи. */
@Composable
private fun FlowPath(up: Boolean, active: Boolean, color: Color, speedMs: Int) {
    val t = rememberInfiniteTransition(label = "flow")
    val phase by t.animateFloat(0f, 1f, infiniteRepeatable(tween(speedMs, easing = LinearEasing)), label = "flow")
    val ring = K.ring
    Canvas(Modifier.fillMaxSize()) {
        val sx = size.width / 100
        val sy = size.height / 100
        val y = if (up) 24f else 76f
        val p = Path().apply {
            moveTo(14 * sx, 50 * sy)
            cubicTo(32 * sx, 50 * sy, 30 * sx, y * sy, 50 * sx, y * sy)
            cubicTo(70 * sx, y * sy, 68 * sx, 50 * sy, 86 * sx, 50 * sy)
        }
        val on = 3.dp.toPx()
        val off = 5.dp.toPx()
        drawPath(
            p, if (active) color else ring.copy(alpha = 0.8f),
            style = Stroke(
                if (active) 2.dp.toPx() else 1.5.dp.toPx(), cap = StrokeCap.Round,
                pathEffect = PathEffect.dashPathEffect(if (active) floatArrayOf(on, off) else floatArrayOf(2.dp.toPx(), off), if (active) -phase * (on + off) * 2 else 0f),
            ),
        )
    }
}

// ── Как вас видят сайты ──────────────────────────────────────

/** «Как вас видят сайты»: адрес, страна и провайдер через VPN и напрямую, утечки IPv6 и DNS. Проверка — по кнопке. */
@Composable
private fun IpCard(m: KlickModel) {
    var report by remember { mutableStateOf(lastIp) }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    fun check() {
        if (busy) return
        busy = true
        scope.launch {
            m.ipCheck()?.let { report = it; lastIp = it }
            busy = false
        }
    }
    KSectionHead("Как вас видят сайты") {
        if (report != null) {
            Row(Modifier.clickable(enabled = !busy) { check() }, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                KIcon(KIcons.refresh, 14.dp, K.text2)
                KText(if (busy) "Проверяю…" else "Проверить", size = 13.sp, weight = FontWeight.Medium, color = K.text2)
            }
        }
    }
    Column(Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard().padding(14.dp)) {
        val r = report
        if (r != null) {
            IpGrid(r)
            KText(
                "Город примерный: базы геолокации расходятся. «Похоже на VPN» — по базе одного сервиса, у других может отличаться. WebRTC — только в браузере. Проверено в ${SimpleDateFormat("HH:mm", Locale.US).format(Date((r.checkedAt * 1000).toLong()))}.",
                size = 12.sp, color = K.dim2, lineHeight = 17.sp, modifier = Modifier.padding(top = 12.dp),
            )
        } else {
            KText("Какой адрес, страну и провайдера видят сайты — через VPN и напрямую. Заодно проверим, не уходят ли IPv6 и DNS мимо туннеля.", size = 13.sp, color = K.text3, lineHeight = 18.sp)
            CardButton(if (busy) "Проверяю…" else "Проверить", KIcons.refresh, enabled = !busy, modifier = Modifier.padding(top = 12.dp)) { check() }
        }
    }
}

/** Последний отчёт живёт, пока открыто приложение. */
private var lastIp: IpReport? = null

private data class Cell(val text: String, val color: Color, val mono: Boolean = false)

@Composable
private fun cells(c: IpColumn?, vpnColumn: Boolean): List<Cell> {
    val dash = Cell("—", K.dim2)
    if (c == null) return listOf(Cell("VPN выключен", K.dim2)) + List(6) { dash }
    if (c.error != null && c.ipv4 == null && c.ipv6 == null) return listOf(Cell("не удалось", K.red)) + List(6) { dash }
    val country = c.countryCode?.uppercase()?.let { Locale("", it).getDisplayCountry(Locale("ru")).ifEmpty { null } } ?: c.country
    val detected = when (c.vpnDetected) {
        null -> dash
        true -> Cell("да", if (vpnColumn) K.orange else K.red)
        false -> Cell("нет", if (vpnColumn) K.accent else K.text)
    }
    return listOf(
        Cell(c.ipv4 ?: "—", K.text, true),
        c.ipv6?.let { Cell(it, K.text, true) } ?: Cell("нет", K.dim2),
        Cell(country ?: "—", K.text),
        Cell(c.city ?: "—", K.text),
        Cell(c.provider ?: "—", K.text),
        c.reverseDns?.let { Cell(it, K.text, true) } ?: dash,
        detected,
    )
}

@Composable
private fun IpGrid(r: IpReport) {
    val labels = listOf("IP", "IPv6", "Страна", "Город, примерно", "Провайдер", "Имя адреса", "Похоже на VPN")
    val a = cells(r.viaVpn, true)
    val b = cells(r.direct, false)
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row {
            Spacer(Modifier.width(92.dp))
            KText("ЧЕРЕЗ VPN", size = 11.sp, color = K.dim, tracking = 0.5.sp, modifier = Modifier.weight(1f))
            KText("НАПРЯМУЮ", size = 11.sp, color = K.dim, tracking = 0.5.sp, modifier = Modifier.weight(1f))
        }
        labels.forEachIndexed { i, label ->
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                KText(label, size = 12.sp, color = K.dim, modifier = Modifier.width(82.dp))
                CellText(a[i], Modifier.weight(1f))
                CellText(b[i], Modifier.weight(1f))
            }
        }
        Box(Modifier.padding(vertical = 2.dp).fillMaxWidth().height(1.dp).background(K.line))
        Flag("Утечка IPv6", r.ipv6Leak, bad = true)
        Flag("DNS через kl!ck", r.dnsProtected, bad = false)
    }
}

@Composable
private fun CellText(c: Cell, modifier: Modifier) {
    KText(c.text, size = if (c.mono) 11.5.sp else 12.sp, color = c.color, lines = 1, mono = c.mono, modifier = modifier)
}

/** Утечка: «есть» — плохо; DNS: «да» — хорошо. */
@Composable
private fun Flag(label: String, v: Boolean?, bad: Boolean) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        KText(label, size = 12.sp, color = K.dim, modifier = Modifier.width(82.dp))
        val text = if (v == null) "—" else if (bad) (if (v) "есть" else "нет") else (if (v) "да" else "нет")
        KText(text, size = 12.sp, color = if (v == null) K.dim2 else if (v == bad) K.red else K.accent, modifier = Modifier.weight(1f))
        Spacer(Modifier.weight(1f))
    }
}
