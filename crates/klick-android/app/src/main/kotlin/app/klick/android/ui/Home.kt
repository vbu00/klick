package app.klick.android.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameMillis
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.Conn
import app.klick.android.KlickModel
import app.klick.android.Srv
import app.klick.android.Texts
import app.klick.android.Toast
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.sqrt

/**
 * Главная — как ui/src/screens/Home.tsx и iOS β (HomeView.swift): статус, кнопка, чип режима, подписки с
 * серверами, скорость. Подписку можно посмотреть, не переключаясь на неё; активной она становится, когда в
 * ней включают VPN или выбирают сервер.
 */
@Composable
fun HomeScreen(m: KlickModel, onPower: (String?) -> Unit, onTab: (Int) -> Unit) {
    val conns = m.connections
    if (conns.isEmpty()) {
        EmptyHome { onTab(TAB_ADD) }
        return
    }
    var viewId by rememberSaveable { mutableStateOf<String?>(null) }
    var expanded by rememberSaveable { mutableStateOf(false) }
    var switchTo by remember { mutableStateOf<String?>(null) }
    var switchServer by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()
    val haptics = LocalHapticFeedback.current

    val view = conns.firstOrNull { it.id == (viewId ?: m.activeId) } ?: conns.first()
    val liveId = if (m.isRunning) m.activeId else null
    val isActiveView = view.id == m.activeId
    val viewingLive = m.isRunning && view.id == liveId
    val list = m.servers[view.id].orEmpty()
    // Пока сервер не выбран, ядро при подключении берёт первый — его и показываем выбранным.
    val current = list.firstOrNull { it.selected } ?: list.firstOrNull { it.name == view.selectedServer } ?: list.firstOrNull()
    val vpn = m.vpn

    LaunchedEffect(m.focusConnection) {
        val id = m.focusConnection ?: return@LaunchedEffect
        viewId = id
        expanded = false
        m.focusConnection = null
        if (vpn == "off" && m.activeId != id) m.selectConnection(id)
    }
    LaunchedEffect(viewId) {
        val id = viewId ?: return@LaunchedEffect
        if (id != m.activeId && m.servers[id] == null) m.viewServers(id)
    }

    fun pickOther(name: String) {
        if (m.isRunning) {
            switchServer = name
            switchTo = view.id
            return
        }
        scope.launch { if (m.selectConnection(view.id)) m.selectServer(name) }
    }

    KScreen(onRefresh = { if (view.kind == "subscription") m.refresh(view.id) else m.reload() }) {
        // Статус и время подключения.
        Column(Modifier.fillMaxWidth().padding(top = 28.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(2.dp)) {
            KText(statusLabel(m, viewingLive), size = 15.sp, color = statusTone(m))
            var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
            LaunchedEffect(m.isRunning) {
                while (m.isRunning) {
                    now = System.currentTimeMillis()
                    delay(1000)
                }
            }
            KText(Texts.timer(if (m.isRunning) m.state?.since ?: 0.0 else 0.0, now / 1000.0), size = 30.sp, weight = FontWeight.Medium, tracking = 0.5.sp, mono = true)
        }

        PowerButton(core(m, viewingLive), wave = viewingLive && vpn == "connected" && m.pending == null, modifier = Modifier.padding(top = 22.dp)) {
            haptics.performHapticFeedback(HapticFeedbackType.LongPress)
            // Второе нажатие, пока движок не ответил на первое, — не команда.
            if (vpn == "connecting" || m.pending != null) return@PowerButton
            if (m.isRunning && !viewingLive) switchTo = view.id else onPower(view.id)
        }

        // Чип режима — на «Соединение».
        Box(Modifier.fillMaxWidth().padding(top = 18.dp), contentAlignment = Alignment.Center) {
            Row(
                Modifier.height(34.dp).clip(CircleShape).background(K.card).press { onTab(TAB_CONNECTION) }.padding(horizontal = 16.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(Modifier.size(7.dp).background(modeDot(m), CircleShape))
                KText(if (m.state?.routing == "all_vpn") "VPN для всего" else "VPN для выбранного", size = 13.sp, color = K.text2)
            }
        }

        when (vpn) {
            "reconnecting" -> Banner("Переподключаюсь… " + attempt(m), "Сервер перестал отвечать, пробую снова")
            "server_down" -> Box(Modifier.clickable { expanded = true }) { Banner("Сервер не отвечает", "Проверяю раз в минуту. Можно выбрать другой сервер") }
            "error" -> Banner("VPN не включился", Texts.error(m.state?.error.orEmpty()), bad = true)
        }

        if (conns.size > 1) {
            Row(Modifier.padding(top = 34.dp).horizontalScroll(rememberScrollState()), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                conns.forEach { c ->
                    PChip(c.name, on = c.id == view.id, live = m.isRunning && c.id == liveId) {
                        haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        expanded = false
                        viewId = c.id
                    }
                }
            }
        }

        ProfileCard(
            m, view, viewingLive, live = vpn == "connected", isActive = isActiveView, list = list, current = current,
            expanded = expanded, onExpanded = { expanded = it }, pinging = m.pinging && isActiveView, onPickOther = ::pickOther,
            modifier = Modifier.padding(top = if (conns.size > 1) 10.dp else 34.dp),
        )

        SpeedBlock(m)
    }

    if (switchTo != null) {
        val liveName = conns.firstOrNull { it.id == liveId }?.name ?: "другое подключение"
        val toName = conns.firstOrNull { it.id == switchTo }?.name ?: "выбранное"
        KDialog(
            title = "Сменить подключение?",
            text = "Сейчас работает $liveName. Текущее соединение разорвётся на пару секунд, и трафик пойдёт через $toName.",
            confirm = "Переключить",
            onConfirm = {
                val id = switchTo!!
                val server = switchServer
                switchTo = null
                switchServer = null
                scope.launch { if (m.selectConnection(id) && server != null) m.selectServer(server) }
            },
            onDismiss = { switchTo = null; switchServer = null },
        )
    }
}

const val TAB_HOME = 0
const val TAB_CONNECTION = 1
const val TAB_ADD = 2
const val TAB_SETTINGS = 3

private fun statusLabel(m: KlickModel, viewingLive: Boolean): String {
    if (m.vpn == "connecting" || m.pending == "on") return "подключаюсь…"
    if (m.pending == "off") return "отключаюсь…"
    return when (m.vpn) {
        "reconnecting" -> "переподключаюсь… " + attempt(m)
        "server_down" -> "сервер не отвечает"
        "error" -> "VPN не включился"
        else -> if (!m.isRunning) "не подключено" else if (viewingLive) "время подключения" else "нажмите, чтобы переключиться сюда"
    }
}

/** «2 из 3» — попытка переподключения. */
private fun attempt(m: KlickModel): String = m.state?.attempt?.takeIf { it.size == 2 }?.let { "${it[0]} из ${it[1]}" }.orEmpty()

@Composable
private fun statusTone(m: KlickModel): Color = when {
    m.pending != null -> K.dim
    m.vpn == "reconnecting" || m.vpn == "server_down" -> K.orange
    m.vpn == "error" -> K.red
    else -> K.dim
}

@Composable
private fun modeDot(m: KlickModel): Color = when {
    m.vpn == "error" -> K.red
    m.vpn == "reconnecting" || m.vpn == "server_down" -> K.orange
    m.isRunning -> K.green
    else -> K.dim
}

enum class Core { Off, Live, Other, Busy, BusyWarn, Warn, Bad }

private fun core(m: KlickModel, viewingLive: Boolean): Core {
    if (m.vpn == "connecting" || m.pending != null) return Core.Busy
    return when (m.vpn) {
        "reconnecting" -> if (viewingLive) Core.BusyWarn else Core.Other
        "server_down" -> if (viewingLive) Core.Warn else Core.Other
        "error" -> Core.Bad
        else -> if (m.isRunning) (if (viewingLive) Core.Live else Core.Other) else Core.Off
    }
}

/** Вопрос с подтверждением в цветах kl!ck. */
@Composable
fun KDialog(title: String, text: String, confirm: String, danger: Boolean = false, onConfirm: () -> Unit, onDismiss: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = K.card,
        title = { KText(title, size = 17.sp, weight = FontWeight.SemiBold) },
        text = { KText(text, size = 14.sp, color = K.text3, lineHeight = 20.sp) },
        confirmButton = { TextButton(onClick = onConfirm) { KText(confirm, weight = FontWeight.SemiBold, color = if (danger) K.red else K.accent) } },
        dismissButton = { TextButton(onClick = onDismiss) { KText("Отмена", color = K.dim) } },
    )
}

/** `.banner`: оранжевая (или красная) плашка с точкой. */
@Composable
private fun Banner(title: String, sub: String, bad: Boolean = false) {
    val tone = if (bad) K.red else K.orange
    Row(
        Modifier.padding(top = 16.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).background(K.card).background(tone.copy(alpha = 0.12f))
            .padding(horizontal = 14.dp, vertical = 11.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.size(7.dp).background(tone, CircleShape))
        Column(verticalArrangement = Arrangement.spacedBy(1.dp)) {
            KText(title, size = 13.sp)
            KText(sub, size = 12.sp, color = K.text3)
        }
    }
}

/** `.pchip`: чип подписки; работающая — с пульсирующей точкой. */
@Composable
private fun PChip(title: String, on: Boolean, live: Boolean, onClick: () -> Unit) {
    Row(
        Modifier.height(30.dp).clip(CircleShape).background(if (on) K.raised else Color.Transparent)
            .border(1.dp, if (on) K.raised else K.line, CircleShape).press(onClick = onClick).padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (live) LiveDot()
        KText(title, size = 12.sp, weight = FontWeight.Medium, color = if (on) K.text else K.dim, lines = 1)
    }
}

/** Точка 6 с пульсом до 42 (`pulse-dot`). */
@Composable
private fun LiveDot() {
    val t = rememberInfiniteTransition(label = "dot")
    val p by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2200, easing = LinearEasing)), label = "dot")
    val green = K.green
    Canvas(Modifier.size(6.dp)) {
        drawCircle(green.copy(alpha = 0.35f * (1 - p)), radius = size.minDimension / 2 * (1 + 6 * p))
        drawCircle(green)
    }
}

// ── Кнопка питания ───────────────────────────────────────────

/** `.power-btn` 120 с кругом 84 и значком 34; когда VPN работает — волна по точкам вокруг. */
@Composable
fun PowerButton(core: Core, wave: Boolean, modifier: Modifier = Modifier, onClick: () -> Unit) {
    val fill by animateColorAsState(if (core == Core.Live) K.accent else K.card, label = "fill")
    val border by animateColorAsState(
        when (core) {
            Core.Off -> K.line
            Core.Live, Core.Other -> K.accent
            Core.Busy -> K.accent.copy(alpha = 0.45f)
            Core.BusyWarn, Core.Warn -> K.orange
            Core.Bad -> K.red
        }, label = "border",
    )
    val icon by animateColorAsState(
        when (core) {
            Core.Off -> K.dim
            Core.Live -> K.onAccent
            Core.Other, Core.Busy -> K.accent
            Core.BusyWarn, Core.Warn -> K.orange
            Core.Bad -> K.red
        }, label = "icon",
    )
    Box(modifier.fillMaxWidth().height(120.dp), contentAlignment = Alignment.Center) {
        AnimatedVisibility(wave, enter = fadeIn(tween(600)), exit = fadeOut(tween(600))) { Wave() }
        Box(Modifier.size(120.dp).clip(CircleShape).press(onClick = onClick), contentAlignment = Alignment.Center) {
            if (core == Core.Live) Pulse()
            Box(Modifier.size(84.dp).background(fill, CircleShape).border(1.dp, border, CircleShape))
            if (core == Core.Busy || core == Core.BusyWarn) Spinner(if (core == Core.BusyWarn) K.orange else K.accent)
            KIcon(KIcons.power, 34.dp, icon)
        }
    }
}

/** Круг акцента 35 % растёт до 120 и гаснет за 2.2 с. */
@Composable
private fun Pulse() {
    val t = rememberInfiniteTransition(label = "pulse")
    val p by t.animateFloat(0f, 1f, infiniteRepeatable(tween(2200, easing = { 1 - (1 - it).pow(2) })), label = "pulse")
    val accent = K.accent
    Canvas(Modifier.size(120.dp)) {
        drawCircle(accent.copy(alpha = 0.35f * (1 - p)), radius = 43.dp.toPx() * (1 + 0.4286f * p))
    }
}

/** Дуга сверху по кругу 92, оборот за секунду. */
@Composable
private fun Spinner(color: Color) {
    val t = rememberInfiniteTransition(label = "spin")
    val turn by t.animateFloat(0f, 360f, infiniteRepeatable(tween(1000, easing = LinearEasing), RepeatMode.Restart), label = "spin")
    Canvas(Modifier.size(92.dp)) {
        drawArc(color, startAngle = -135f + turn, sweepAngle = 90f, useCenter = false, style = Stroke(2.dp.toPx(), cap = StrokeCap.Round))
    }
}

/**
 * Сетка точек 14 вокруг кнопки (гаснет к краям), по ней раз в 5.2 с расходится волна акцента, а от круга —
 * кольцо. 30 кадров в секунду — глазу хватает, батарее легче.
 */
@Composable
private fun Wave() {
    var t by remember { mutableDoubleStateOf(0.0) }
    LaunchedEffect(Unit) {
        val start = withFrameMillis { it }
        while (true) {
            withFrameMillis { now -> t = ((now - start) % 5200) / 5200.0 }
            delay(33)
        }
    }
    val dots = K.dots
    val accent = K.accent
    Canvas(Modifier.fillMaxWidth().height(360.dp).padding(horizontal = 0.dp)) {
        val c = Offset(size.width / 2, size.height / 2)
        val rx = size.width / 2 * sqrt(2f)
        val ry = size.height / 2 * sqrt(2f)
        val far = sqrt(c.x * c.x + c.y * c.y)
        val ease = { x: Double -> 1 - (1 - x).pow(2.2) }
        val scale = 3.4 * ease(min(1.0, t / 0.55))
        val alpha = if (t < 0.55) 1.0 else if (t < 0.62) 1 - (t - 0.55) / 0.07 else 0.0
        val step = 14.dp.toPx()
        val nx = (size.width / step / 2).toInt() + 1
        val ny = (size.height / step / 2).toInt() + 1
        for (iy in -ny..ny) for (ix in -nx..nx) {
            val x = c.x + ix * step
            val y = c.y + iy * step
            val dx = (x - c.x) / rx
            val dy = (y - c.y) / ry
            val e = sqrt(dx * dx + dy * dy).toDouble()
            val mask = if (e <= 0.2) 1.0 else if (e >= 0.72) 0.0 else 1 - (e - 0.2) / 0.52
            if (mask <= 0) continue
            drawCircle(dots.copy(alpha = mask.toFloat()), radius = 1.25.dp.toPx(), center = Offset(x, y))
            if (scale <= 0 || alpha <= 0) continue
            val r = sqrt((x - c.x) * (x - c.x) + (y - c.y) * (y - c.y)) / far / scale
            val lit = band(r)
            if (lit <= 0) continue
            drawCircle(accent.copy(alpha = (mask * lit * alpha).toFloat()), radius = 1.6.dp.toPx(), center = Offset(x, y))
        }
        // Кольцо от круга: 84, 1.5, акцент 55 %; 0.55 → 3.2 к 55 %, видно с 8 %.
        val ringScale = 0.55 + (3.2 - 0.55) * ease(min(1.0, t / 0.55))
        val ringAlpha = if (t < 0.08) 0.7 * t / 0.08 else if (t < 0.55) 0.7 * (1 - (t - 0.08) / 0.47) else 0.0
        if (ringAlpha > 0) {
            drawCircle(accent.copy(alpha = (0.55 * ringAlpha).toFloat()), radius = 42.dp.toPx() * ringScale.toFloat(), center = c, style = Stroke(1.5.dp.toPx()))
        }
    }
}

/** Яркость точки в кольце волны: `transparent 0 24%, accent 30% 40%, transparent 48%`. */
private fun band(r: Double): Double = when {
    r < 0.24 || r >= 0.48 -> 0.0
    r < 0.30 -> (r - 0.24) / 0.06
    r <= 0.40 -> 1.0
    else -> 1 - (r - 0.40) / 0.08
}

// ── Карточка подписки ────────────────────────────────────────

/**
 * `.pcard`: значок, название и сервер, строка «трафик · срок»; «Обновить» и ссылки панели — в меню ⋯
 * (обновить можно и потянув экран вниз); по нажатию — серверы.
 */
@Composable
private fun ProfileCard(
    m: KlickModel, conn: Conn, viewingLive: Boolean, live: Boolean, isActive: Boolean, list: List<Srv>, current: Srv?,
    expanded: Boolean, onExpanded: (Boolean) -> Unit, pinging: Boolean, onPickOther: (String) -> Unit, modifier: Modifier = Modifier,
) {
    val ctx = LocalContext.current
    val scope = rememberCoroutineScope()
    val haptics = LocalHapticFeedback.current
    val isSub = conn.kind == "subscription"
    var menu by remember { mutableStateOf(false) }
    var refreshing by remember { mutableStateOf(false) }
    var confirmRemove by remember { mutableStateOf(false) }

    fun toggle() {
        haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
        val open = !expanded
        onExpanded(open)
        if (open && isActive && list.size > 1 && list.all { it.delay == null }) m.testLatency()
        if (open && !isActive && m.servers[conn.id] == null) scope.launch { m.viewServers(conn.id) }
    }

    Column(modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard()) {
        Row(Modifier.fillMaxWidth().clickable { toggle() }.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp)) {
            Box(Modifier.size(52.dp).clip(RoundedCornerShape(K.r(16))).background(K.elem), contentAlignment = Alignment.Center) {
                KIcon(if (isSub) KIcons.globe else KIcons.rules, 24.dp, K.dim)
            }
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    KText(conn.name, size = 17.sp, weight = FontWeight.SemiBold, lines = 1, modifier = Modifier.weight(1f, fill = false))
                    if (viewingLive && live) {
                        Box(Modifier.height(20.dp).clip(RoundedCornerShape(6.dp)).background(K.green.copy(alpha = 0.16f)).padding(horizontal = 8.dp), contentAlignment = Alignment.Center) {
                            KText("Работает", size = 11.sp, weight = FontWeight.SemiBold, color = K.green)
                        }
                    }
                }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    KText(current?.name ?: conn.selectedServer ?: if (isSub) "Подписка" else "Одиночная конфигурация", size = 13.sp, color = K.dim, lines = 1, modifier = Modifier.weight(1f, fill = false))
                    // Задержку показываем, только когда её измерили: «нет ответа» до проверки было бы неправдой.
                    if (pinging || current?.delay != null) {
                        Box(Modifier.size(3.dp).background(K.dim2, CircleShape))
                        KText(if (pinging) "…" else Texts.ping(current?.delay), size = 13.sp, mono = true, color = if (pinging) K.dim else K.ping(current?.delay))
                    }
                }
                if (isSub) meta(conn, refreshing)?.let { (text, ending) ->
                    KText(text, size = 12.sp, color = if (ending) K.orange else K.dim, lines = 1, mono = true, modifier = Modifier.padding(top = 2.dp))
                }
            }
            Box {
                Box(Modifier.size(32.dp).clip(CircleShape).clickable { menu = true }, contentAlignment = Alignment.Center) { KIcon(KIcons.dots, 20.dp, K.dim) }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }, containerColor = K.raised) {
                    if (isSub) {
                        MenuItem("Обновить подписку", KIcons.refresh, enabled = !refreshing) {
                            menu = false
                            scope.launch {
                                refreshing = true
                                m.refresh(conn.id)
                                refreshing = false
                            }
                        }
                        listOfNotNull(conn.info?.supportUrl?.let { "Поддержка" to it }, conn.info?.pageUrl?.let { "Страница подписки" to it }).forEach { (title, url) ->
                            MenuItem(title, KIcons.globe) {
                                menu = false
                                runCatching { ctx.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) }
                            }
                        }
                    }
                    if (isActive) MenuItem("Проверить задержку", KIcons.server) {
                        menu = false
                        onExpanded(true)
                        m.testLatency()
                    }
                    HorizontalDivider(color = K.line)
                    MenuItem(if (isSub) "Удалить подписку" else "Удалить конфигурацию", KIcons.trash, danger = true) {
                        menu = false
                        confirmRemove = true
                    }
                }
            }
        }
        if (isSub && !conn.info?.announce.isNullOrEmpty()) {
            KText(conn.info!!.announce!!, size = 12.sp, color = K.dim, modifier = Modifier.padding(start = 16.dp, end = 16.dp, bottom = 16.dp))
        }
        AnimatedVisibility(expanded, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut()) {
            Servers(m, conn, isActive, list, current, pinging, onPickOther)
        }
    }

    if (confirmRemove) {
        KDialog(
            title = if (isSub) "Удалить подписку?" else "Удалить конфигурацию?", text = conn.name, confirm = "Удалить", danger = true,
            onConfirm = { confirmRemove = false; m.remove(conn.id) }, onDismiss = { confirmRemove = false },
        )
    }
}

@Composable
private fun MenuItem(title: String, icon: Int, enabled: Boolean = true, danger: Boolean = false, onClick: () -> Unit) {
    DropdownMenuItem(
        text = { KText(title, color = if (danger) K.red else if (enabled) K.text else K.dim) },
        leadingIcon = { KIcon(icon, 18.dp, if (danger) K.red else K.dim) },
        enabled = enabled, onClick = onClick,
    )
}

/**
 * Одна строка под сервером: «35.0 ГБ · без лимита · бессрочно», «12.3 из 50 ГБ · до 12 ноября». Оранжевая,
 * когда подписка кончается: трафика меньше 10 % или осталось 3 дня и меньше.
 */
private fun meta(conn: Conn, refreshing: Boolean): Pair<String, Boolean>? {
    if (refreshing) return "Обновляем подписку…" to false
    val info = conn.info ?: return null
    val used = info.used
    val total = info.total ?: 0.0
    val parts = mutableListOf<String>()
    if (total > 0) parts += Texts.traffic(used, total) else if (used > 0) parts += "${Texts.used(used)} · без лимита"
    var ending = total > 0 && used / total > 0.9
    val expire = info.expire
    if (expire != null) {
        val days = Texts.daysLeft(expire)
        ending = ending || days <= 3
        parts += if (days == 0) "истекает сегодня" else "до ${Texts.date(expire)}"
    } else if (parts.isNotEmpty()) {
        parts += "бессрочно"
    }
    return if (parts.isEmpty()) null else parts.joinToString(" · ") to ending
}

@Composable
private fun Servers(m: KlickModel, conn: Conn, isActive: Boolean, list: List<Srv>, current: Srv?, pinging: Boolean, onPickOther: (String) -> Unit) {
    Column(Modifier.fillMaxWidth()) {
        Box(Modifier.fillMaxWidth().height(1.dp).background(K.line))
        Column(Modifier.padding(start = 8.dp, end = 8.dp, top = 12.dp, bottom = 8.dp)) {
            Row(Modifier.fillMaxWidth().padding(start = 8.dp, end = 8.dp, bottom = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                KCaps("Серверы · ${list.size}", Modifier.weight(1f))
                if (isActive) ElemButton(if (pinging) "Проверяем…" else "Проверить задержку", enabled = !pinging) { m.testLatency() }
            }
            if (!isActive && list.isNotEmpty()) {
                KText("Выберите сервер — kl!ck перейдёт на это подключение.", size = 12.sp, color = K.dim, modifier = Modifier.padding(start = 8.dp, end = 8.dp, bottom = 8.dp))
            }
            if (list.isEmpty()) {
                KText(
                    if (isActive || m.servers[conn.id] != null) "Серверов пока нет. Обновите подписку — список придёт от панели." else "Загружаю список серверов…",
                    size = 12.sp, color = K.dim, modifier = Modifier.padding(start = 8.dp, end = 8.dp, top = 4.dp, bottom = 8.dp),
                )
            }
            list.forEach { s -> ServerRow(m, s, on = s.name == current?.name, isActive, pinging, onPickOther) }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ServerRow(m: KlickModel, s: Srv, on: Boolean, isActive: Boolean, pinging: Boolean, onPickOther: (String) -> Unit) {
    val haptics = LocalHapticFeedback.current
    val ctx = LocalContext.current
    Row(
        Modifier.padding(bottom = 4.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(12))).background(if (on) K.hover else Color.Transparent)
            .combinedClickable(
                onClick = {
                    haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                    if (!isActive) onPickOther(s.name) else if (!on) m.selectServer(s.name)
                },
                onLongClick = {
                    // Долгое нажатие — скопировать имя, как пункт контекстного меню у iOS.
                    haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                    (ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("kl!ck", s.name))
                    m.toast = Toast("Имя скопировано", s.name)
                },
            )
            .padding(horizontal = 8.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        KRadio(on)
        Column(Modifier.weight(1f)) {
            KText(s.name, weight = FontWeight.Medium, lines = 1)
            KText(Texts.protocol(s.kind), size = 12.sp, color = K.dim)
        }
        s.speed?.let { KText(Texts.mbps(it), size = 11.sp, color = K.dim, mono = true) }
        KText(if (pinging) "…" else Texts.ping(s.delay), size = 13.sp, mono = true, color = if (pinging) K.dim else K.ping(s.delay))
    }
}

// ── Скорость ─────────────────────────────────────────────────

/** Замер скорости, плитки «Чтение» и «Загрузка» и график за 60 секунд. */
@Composable
private fun SpeedBlock(m: KlickModel) {
    val connected = m.vpn == "connected" && m.pending == null
    Column {
        if (connected || m.speed != null) {
            Row(
                Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(18))).kCard(18).padding(horizontal = 14.dp, vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                    val r = m.speed
                    when {
                        m.speedTesting -> KText("Меряю скорость через сервер… до полуминуты", size = 12.sp, color = K.dim, lines = 1)
                        r != null -> {
                            KText("↓ ${r.down?.let(Texts::mbps) ?: "—"} · ↑ ${r.up?.let(Texts::mbps) ?: "—"}", weight = FontWeight.SemiBold, mono = true, lines = 1)
                            r.server?.let { KText(it, size = 12.sp, color = K.dim, lines = 1) }
                        }
                        else -> KText("Сколько реально даёт сервер", size = 12.sp, color = K.dim, lines = 1)
                    }
                }
                ElemButton(if (m.speedTesting) "Меряю…" else if (m.speed != null) "Ещё раз" else "Проверить скорость", KIcons.refresh, enabled = !m.speedTesting && connected) { m.speedTest() }
            }
        }
        Row(Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Tile("Чтение", KIcons.download, m.traffic.down, m.traffic.downSum, Modifier.weight(1f))
            Tile("Загрузка", KIcons.upload, m.traffic.up, m.traffic.upSum, Modifier.weight(1f))
        }
        Graph(m.traffic.downHist, m.traffic.upHist)
    }
}

@Composable
private fun Tile(title: String, icon: Int, rate: Double, sum: Double, modifier: Modifier) {
    Column(modifier.clip(RoundedCornerShape(K.r(20))).kCard().padding(16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            KText(title, color = K.dim, modifier = Modifier.weight(1f))
            KIcon(icon, 22.dp, K.dim)
        }
        KText(Texts.rate(rate), size = 22.sp, weight = FontWeight.SemiBold, mono = true, lines = 1, modifier = Modifier.padding(top = 6.dp))
        KText("всего ${Texts.bytes(sum)}", size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 2.dp))
    }
}

@Composable
private fun Graph(down: List<Double>, up: List<Double>) {
    val peak = down.maxOrNull() ?: 0.0
    Column(Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard().padding(16.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            KText("Последние 60 секунд", size = 13.sp, color = K.dim, modifier = Modifier.weight(1f))
            Legend("Чтение", K.accent)
            Spacer(Modifier.width(12.dp))
            Legend("Загрузка", K.dim)
        }
        val accent = K.accent
        val dim = K.dim
        val elem = K.elem
        Canvas(Modifier.padding(top = 12.dp).fillMaxWidth().height(110.dp)) {
            val maxV = max(50.0, max(down.maxOrNull() ?: 0.0, up.maxOrNull() ?: 0.0)) * 1.1
            val sx = size.width / 300
            val sy = size.height / 90
            drawLine(elem, Offset(0f, 89 * sy), Offset(size.width, 89 * sy), 1.dp.toPx())
            drawLine(elem, Offset(0f, 45 * sy), Offset(size.width, 45 * sy), 1.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(3.dp.toPx(), 4.dp.toPx())))
            fun line(h: List<Double>) = Path().apply {
                h.forEachIndexed { i, v ->
                    val x = (i.toFloat() / max(1, h.size - 1) * 300) * sx
                    val y = (88 - v / maxV * 84).toFloat() * sy
                    if (i == 0) moveTo(x, y) else lineTo(x, y)
                }
            }
            val d = line(down)
            val area = Path().apply {
                addPath(d)
                lineTo(size.width, 89 * sy)
                lineTo(0f, 89 * sy)
                close()
            }
            drawPath(area, accent.copy(alpha = 0.12f))
            drawPath(d, accent, style = Stroke(2.dp.toPx(), join = StrokeJoin.Round))
            drawPath(line(up), dim, style = Stroke(1.5.dp.toPx(), join = StrokeJoin.Round))
        }
        Row(Modifier.padding(top = 6.dp)) {
            KText("−60 с", size = 11.sp, color = K.dim2)
            Spacer(Modifier.weight(1f))
            KText("Пик: ${Texts.rate(peak * 1e6 / 8)}", size = 11.sp, color = K.dim2)
            Spacer(Modifier.weight(1f))
            KText("сейчас", size = 11.sp, color = K.dim2)
        }
    }
}

@Composable
private fun Legend(title: String, color: Color) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        Box(Modifier.size(8.dp, 3.dp).background(color, RoundedCornerShape(2.dp)))
        KText(title, size = 12.sp, color = K.dim)
    }
}

// ── Пусто ────────────────────────────────────────────────────

/** Подключений нет: значок, три шага и «Добавить подключение». */
@Composable
private fun EmptyHome(onAdd: () -> Unit) {
    KScreen {
        Column(Modifier.fillMaxWidth().padding(top = 36.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Box(Modifier.size(88.dp).background(K.card, CircleShape), contentAlignment = Alignment.Center) { KIcon(KIcons.power, 38.dp, K.dim2) }
            KText("Нет подключений", size = 20.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 22.dp))
            KText(
                "Добавьте ссылку на подписку или одиночную конфигурацию — и можно включать.", size = 13.sp, color = K.text3, center = true,
                lineHeight = 18.sp, modifier = Modifier.padding(top = 6.dp),
            )
            Column(Modifier.padding(top = 26.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Step(1, "Вставьте ссылку", "Подписка (https://…) или vless://, vmess://, trojan://, ss://")
                Step(2, "Выберите, что пускать в VPN", "Весь трафик или только нужные сайты и сервисы")
                Step(3, "Нажмите кнопку питания", "Заблокированные сервисы пойдут через VPN, остальное — напрямую")
            }
            PrimaryButton("Добавить подключение", modifier = Modifier.padding(top = 22.dp), onClick = onAdd)
        }
    }
}

@Composable
private fun Step(n: Int, title: String, text: String) {
    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).padding(14.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(26.dp).background(K.elem, CircleShape), contentAlignment = Alignment.Center) {
            KText("$n", size = 12.sp, weight = FontWeight.SemiBold, color = K.text2)
        }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            KText(title, weight = FontWeight.SemiBold)
            KText(text, size = 12.sp, color = K.text3)
        }
    }
}

