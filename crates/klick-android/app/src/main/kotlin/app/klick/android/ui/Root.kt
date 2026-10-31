package app.klick.android.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.KlickModel
import app.klick.android.R
import kotlinx.coroutines.delay

/** Окно kl!ck — как iOS β (RootView): шапка, вкладка, внизу плашки и панель вкладок. */
@Composable
fun Root(m: KlickModel, incoming: String?, onIncomingUsed: () -> Unit, onPower: (String?) -> Unit) {
    var tab by rememberSaveable { mutableIntStateOf(TAB_HOME) }
    // Ссылка из «Поделиться» или klick://add — на «Добавить».
    LaunchedEffect(incoming) { if (incoming != null) tab = TAB_ADD }

    Box(Modifier.fillMaxSize().background(K.win)) {
        Column(Modifier.fillMaxSize().statusBarsPadding().imePadding()) {
            TitleBar(m)
            Box(Modifier.weight(1f)) {
                when {
                    m.startError != null -> Column(Modifier.fillMaxSize().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.Center) {
                        KText("Движок kl!ck не запустился", size = 15.sp, weight = FontWeight.SemiBold)
                        KText(m.startError.orEmpty(), size = 12.sp, color = K.dim, center = true, modifier = Modifier.padding(top = 8.dp))
                    }
                    !m.up -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = K.dim, strokeWidth = 2.dp) }
                    else -> when (tab) {
                        TAB_HOME -> HomeScreen(m, onPower) { tab = it }
                        TAB_CONNECTION -> ConnectionScreen(m)
                        TAB_ADD -> AddScreen(m, incoming, onIncomingUsed) { tab = it }
                        else -> SettingsScreen(m)
                    }
                }
            }
        }
        BottomBar(m, tab) { tab = it }
    }
}

/** `.titlebar`: значок состояния (клавиша kl!ck в цвете состояния) и «kl!ck», 44, линия снизу. */
@Composable
private fun TitleBar(m: KlickModel) {
    val icon = when {
        m.startError != null -> R.drawable.k_status_error
        m.vpn == "connected" -> R.drawable.k_status_success
        m.vpn in setOf("connecting", "reconnecting", "server_down") -> R.drawable.k_status_warning
        m.vpn == "error" -> R.drawable.k_status_error
        m.up && m.connections.isEmpty() -> R.drawable.k_status_no_connection
        else -> R.drawable.k_status_default
    }
    Column {
        Row(Modifier.fillMaxWidth().height(44.dp).padding(start = 16.dp, end = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Image(painterResource(icon), null, Modifier.size(18.dp).clip(RoundedCornerShape(4.dp)))
            KText("kl!ck", weight = FontWeight.Bold, modifier = Modifier.padding(start = 8.dp))
        }
        Box(Modifier.fillMaxWidth().height(1.dp).background(K.line))
    }
}

/** `.nav` и `.toasts`: плавающая капсула вкладок внизу, уведомления над ней, под панелью — затухание. */
@Composable
private fun androidx.compose.foundation.layout.BoxScope.BottomBar(m: KlickModel, tab: Int, onTab: (Int) -> Unit) {
    val win = K.win
    Box(
        Modifier.align(Alignment.BottomCenter).fillMaxWidth().height(140.dp)
            .background(Brush.verticalGradient(0f to win.copy(alpha = 0f), 0.45f to win.copy(alpha = 0.85f), 1f to win)),
    )
    Column(
        Modifier.align(Alignment.BottomCenter).fillMaxWidth().navigationBarsPadding().padding(start = 16.dp, end = 16.dp, bottom = 8.dp),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        ToastView(m)
        TabBar(tab, onTab)
    }
}

/** 4 кнопки 52×44 в капсуле, выбранная подсвечена `--raised`. */
@Composable
private fun TabBar(tab: Int, onTab: (Int) -> Unit) {
    val haptics = LocalHapticFeedback.current
    val items = listOf(KIcons.home to "Главная", KIcons.rules to "Соединение", KIcons.plus to "Добавить", KIcons.settings to "Настройки")
    Row(
        Modifier.shadow(15.dp, CircleShape, ambientColor = Color.Black.copy(alpha = 0.45f), spotColor = Color.Black.copy(alpha = 0.45f))
            .clip(CircleShape).background(K.card.copy(alpha = 0.96f)).border(1.dp, K.line, CircleShape).padding(6.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        items.forEachIndexed { i, (icon, title) ->
            Box(
                Modifier.size(52.dp, 44.dp).clip(CircleShape).background(if (tab == i) K.raised else Color.Transparent)
                    .clickable {
                        if (tab != i) haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onTab(i)
                    },
                contentAlignment = Alignment.Center,
            ) { KIcon(icon, 24.dp, if (tab == i) K.text else K.dim) }
        }
    }
}

/** `.toast`: светлая плашка на тёмной теме (фон — цвет текста), точка цвета события, заголовок и подпись. */
@Composable
private fun ToastView(m: KlickModel) {
    val toast = m.toast
    LaunchedEffect(toast?.id) {
        if (toast != null) {
            delay(3500)
            if (m.toast?.id == toast.id) m.toast = null
        }
    }
    AnimatedVisibility(toast != null, enter = slideInVertically { it } + fadeIn(), exit = slideOutVertically { it } + fadeOut()) {
        val t = toast ?: return@AnimatedVisibility
        Row(
            Modifier.fillMaxWidth().heightIn(min = 48.dp).shadow(12.dp, RoundedCornerShape(K.r(14))).clip(RoundedCornerShape(K.r(14))).background(K.text)
                .clickable { m.toast = null }.padding(start = 14.dp, end = 10.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Box(Modifier.size(8.dp).background(when (t.tone) { "bad" -> K.red; "warn" -> K.orange; "dim" -> K.dim; else -> K.accent }, CircleShape))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
                KText(t.title, size = 13.sp, weight = FontWeight.SemiBold, color = K.win, lines = 1)
                if (!t.sub.isNullOrEmpty()) KText(t.sub, size = 12.sp, color = K.win.copy(alpha = 0.65f), lines = 1)
            }
        }
    }
}

/** «Соединение» — переносится следующим шагом (iOS β: ConnectionScreen.swift). */
@Composable
fun ConnectionScreen(m: KlickModel) {
    KScreen {
        KTitle("Соединение", xl = true)
        KSectionHead("Куда направлять")
        KCardList {
            KRow("VPN для выбранного", "Через VPN — заблокированное и ваш список, остальное напрямую") { KRadio(m.routing == "selected") }
            KDivider()
            KRow("VPN для всего", "Весь трафик через VPN, кроме исключений") { KRadio(m.routing == "all_vpn") }
        }
        KText("Списки, правила и «Сейчас в сети» — в следующей сборке.", size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 12.dp))
    }
}

/** «Настройки» — переносятся следующим шагом (iOS β: SettingsView.swift). */
@Composable
fun SettingsScreen(m: KlickModel) {
    KScreen {
        KTitle("Настройки")
        KSectionHead("Оформление")
        KCardList { KRow("Тема", Palette.bases.firstOrNull { it.first == m.appearance.base }?.second ?: "Системная", chevron = true) }
        KText("Остальные настройки — в следующей сборке.", size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 12.dp))
        Spacer(Modifier.height(1.dp))
    }
}
