package app.klick.android.ui

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.BypassApps
import app.klick.android.KlickModel
import app.klick.android.Texts
import app.klick.android.TunnelService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext

/** Карточка на «Соединении»: сколько приложений мимо VPN и первые значки. */
@Composable
fun AppsCard(onClick: () -> Unit) {
    val ctx = LocalContext.current
    val n = remember { BypassApps.load(ctx).size }
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).clickable(onClick = onClick).padding(16.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Column(Modifier.weight(1f)) {
            KText("Приложения мимо VPN", size = 16.sp, weight = FontWeight.SemiBold)
            KText(
                if (n == 0) "Банк, Госуслуги, игра с проверкой страны — пойдут напрямую" else Texts.plural(n, "приложение", "приложения", "приложений") + " напрямую, остальные — по правилам выше",
                size = 12.5.sp, color = K.dim, lineHeight = 17.sp, modifier = Modifier.padding(top = 3.dp),
            )
        }
        KIcon(KIcons.chevron, 14.dp, K.dim)
    }
}

/**
 * Выбор приложений мимо VPN: поиск, выбранные сверху. Пока VPN включён, список применяется сам через
 * секунду после последнего переключения — туннель пересобирается без обрыва.
 */
@Composable
fun AppsScreen(m: KlickModel, onBack: () -> Unit) {
    val ctx = LocalContext.current
    val iconPx = with(LocalDensity.current) { 36.dp.roundToPx() }
    var apps by remember { mutableStateOf<List<BypassApps.App>?>(null) }
    var picked by remember { mutableStateOf(BypassApps.load(ctx)) }
    // Порядок фиксируем при открытии: выбранные сверху, но строка не прыгает из-под пальца при нажатии.
    val initial = remember { picked }
    var query by remember { mutableStateOf("") }
    var dirty by remember { mutableStateOf(false) }
    LaunchedEffect(Unit) { apps = withContext(Dispatchers.IO) { BypassApps.installed(ctx, iconPx) } }
    LaunchedEffect(picked) {
        if (!dirty) return@LaunchedEffect
        withContext(Dispatchers.IO) { BypassApps.save(ctx, picked) }
        delay(1000)
        if (m.isRunning) TunnelService.send(ctx, TunnelService.ACTION_APPS)
    }

    KScreen {
        BackButton("Соединение", onBack)
        KText("Мимо VPN", size = 24.sp, weight = FontWeight.SemiBold, modifier = Modifier.padding(top = 12.dp))
        KText(
            "Отмеченные приложения ходят в интернет напрямую при любом положении «Куда направлять». Удобно для банков и сервисов, которые не любят зарубежные адреса.",
            size = 13.sp, color = K.dim, lineHeight = 18.sp, modifier = Modifier.padding(top = 6.dp),
        )
        Box(
            Modifier.padding(top = 14.dp).fillMaxWidth().height(44.dp).clip(RoundedCornerShape(K.r(12))).kCard(12)
                .border(1.dp, K.line, RoundedCornerShape(K.r(12))).padding(horizontal = 14.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            if (query.isEmpty()) KText("Найти приложение", color = K.dim2)
            BasicTextField(
                query, { query = it }, Modifier.fillMaxWidth(), singleLine = true,
                textStyle = TextStyle(fontSize = 14.sp, color = K.text), cursorBrush = SolidColor(K.accent),
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
            )
        }
        val list = apps
        if (list == null) {
            Box(Modifier.fillMaxWidth().padding(top = 40.dp), contentAlignment = Alignment.Center) { CircularProgressIndicator(color = K.dim, strokeWidth = 2.dp) }
            return@KScreen
        }
        val q = query.trim().lowercase()
        val shown = list.filter { q.isEmpty() || q in it.label.lowercase() || q in it.pkg }
        val groups = listOf(
            "Напрямую" to shown.filter { it.pkg in initial },
            "Через VPN" to shown.filter { it.pkg !in initial },
        ).filter { it.second.isNotEmpty() }
        groups.forEach { (title, rows) ->
            KSectionHead(title)
            Column(Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14)) {
                rows.forEachIndexed { i, app ->
                    if (i > 0) KDivider(inset = 62.dp)
                    val on = app.pkg in picked
                    Row(
                        Modifier.fillMaxWidth().padding(start = 14.dp, end = 10.dp, top = 9.dp, bottom = 9.dp),
                        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        val icon = app.icon
                        if (icon != null) Image(icon.asImageBitmap(), null, Modifier.size(36.dp))
                        else Box(Modifier.size(36.dp).background(K.elem, RoundedCornerShape(K.r(9))))
                        Column(Modifier.weight(1f)) {
                            KText(app.label, weight = FontWeight.SemiBold, lines = 1)
                            KText(if (on) "напрямую" else "через VPN", size = 12.sp, color = if (on) K.text2 else K.accent, lines = 1)
                        }
                        // Тумблер, как везде: включён — через VPN.
                        KToggle(!on) { vpn ->
                            dirty = true
                            picked = if (vpn) picked - app.pkg else picked + app.pkg
                        }
                    }
                }
            }
        }
        if (shown.isEmpty()) {
            Box(Modifier.padding(top = 14.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).padding(22.dp), contentAlignment = Alignment.Center) {
                KText("Ничего не нашлось", size = 13.sp, color = K.dim)
            }
        }
        KText(
            if (m.isRunning) "Изменения применяются сами — VPN не отключается." else "Применится при подключении.",
            size = 12.sp, color = K.dim2, modifier = Modifier.padding(top = 12.dp).offset(x = 4.dp),
        )
    }
}
