package app.klick.android.ui

import androidx.annotation.DrawableRes
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.scale
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.R
import kotlinx.coroutines.launch

// Компоненты — как у веб-версии (ui/src/styles) и iOS β (Theme.swift): те же размеры, отступы и имена.

/** Значки веб-версии (ui/src/assets/icons → res/drawable/k_*.xml, scripts/android/icons.py). */
object KIcons {
    val home = R.drawable.k_home
    val rules = R.drawable.k_rules
    val plus = R.drawable.k_plus
    val settings = R.drawable.k_settings
    val power = R.drawable.k_power
    val globe = R.drawable.k_globe
    val dots = R.drawable.k_dots
    val chevron = R.drawable.k_chevron
    val download = R.drawable.k_download
    val upload = R.drawable.k_upload
    val refresh = R.drawable.k_refresh
    val server = R.drawable.k_server
    val copy = R.drawable.k_copy
    val trash = R.drawable.k_trash
    val info = R.drawable.k_info
    val qr = R.drawable.k_qr
}

/** Значок цветом текста, как `currentColor`. */
@Composable
fun KIcon(@DrawableRes icon: Int, size: Dp = 16.dp, tint: Color = K.text, modifier: Modifier = Modifier) {
    Icon(painterResource(icon), contentDescription = null, tint = tint, modifier = modifier.size(size))
}

/** Текст kl!ck: размер, вес и цвет одной строкой, как в стилях веб-версии. */
@Composable
fun KText(
    text: String, size: TextUnit = 14.sp, color: Color = K.text, weight: FontWeight = FontWeight.Normal,
    modifier: Modifier = Modifier, lines: Int = Int.MAX_VALUE, tracking: TextUnit = TextUnit.Unspecified, mono: Boolean = false,
    center: Boolean = false, lineHeight: TextUnit = TextUnit.Unspecified,
) {
    Text(
        text, modifier = modifier, color = color, maxLines = lines, overflow = TextOverflow.Ellipsis,
        style = TextStyle(
            fontSize = size, fontWeight = weight, letterSpacing = tracking, lineHeight = lineHeight,
            fontFeatureSettings = if (mono) "tnum" else null,
            textAlign = if (center) androidx.compose.ui.text.style.TextAlign.Center else null,
        ),
    )
}

/** Нажатие уменьшает на [to], как `:active { transform: scale(0.96) }`, с откликом вибрацией. */
@Composable
fun Modifier.press(enabled: Boolean = true, to: Float = 0.96f, haptic: HapticFeedbackType? = null, onClick: () -> Unit): Modifier {
    val source = remember { MutableInteractionSource() }
    val pressed by source.collectIsPressedAsState()
    val scale by animateFloatAsState(if (pressed && enabled) to else 1f, spring(dampingRatio = 0.7f, stiffness = Spring.StiffnessMediumLow), label = "press")
    val haptics = LocalHapticFeedback.current
    return this
        .scale(scale)
        .clickable(source, indication = null, enabled = enabled) {
            haptic?.let(haptics::performHapticFeedback)
            onClick()
        }
}

/** Карточка `--card`: скругление 20 (карточки и списки), 14 — мелкие. */
@Composable
fun Modifier.kCard(radius: Int = 20, color: Color = K.card): Modifier = background(color, RoundedCornerShape(K.r(radius)))

/**
 * Экран: поля 16, снизу место под панель вкладок, колонка до 560 на планшете. [onRefresh] — потянуть вниз.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun KScreen(onRefresh: (suspend () -> Unit)? = null, content: @Composable ColumnScope.() -> Unit) {
    val body = @Composable {
        BoxWithConstraints(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
            Column(
                Modifier.widthIn(max = 560.dp).fillMaxWidth().verticalScroll(rememberScrollState())
                    .padding(start = 16.dp, end = 16.dp, bottom = 120.dp),
                content = content,
            )
        }
    }
    if (onRefresh == null) {
        body()
        return
    }
    var refreshing by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    PullToRefreshBox(isRefreshing = refreshing, onRefresh = {
        scope.launch {
            refreshing = true
            onRefresh()
            refreshing = false
        }
    }) { body() }
}

/** `.screen-title`: 24 полужирный, сверху 14; `xl` — 28 жирный, сверху 8. */
@Composable
fun KTitle(text: String, xl: Boolean = false) {
    KText(text, size = if (xl) 28.sp else 24.sp, weight = if (xl) FontWeight.Bold else FontWeight.SemiBold, modifier = Modifier.padding(top = if (xl) 8.dp else 14.dp))
}

/** `.caps`: 12, заглавными, разрядка 0.6. */
@Composable
fun KCaps(text: String, modifier: Modifier = Modifier) {
    KText(text.uppercase(), size = 12.sp, color = K.dim, tracking = 0.6.sp, modifier = modifier)
}

/** `.sec-head`: подпись раздела над списком, сверху 22; справа — необязательное действие. */
@Composable
fun KSectionHead(title: String, trailing: @Composable RowScope.() -> Unit = {}) {
    Row(Modifier.fillMaxWidth().padding(top = 22.dp).heightIn(min = 24.dp), verticalAlignment = Alignment.CenterVertically) {
        KCaps(title)
        Spacer(Modifier.weight(1f))
        trailing()
    }
}

/** `.card-list`: карточка списка строк, сверху 10. */
@Composable
fun KCardList(top: Dp = 10.dp, content: @Composable ColumnScope.() -> Unit) {
    Column(Modifier.padding(top = top).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard(), content = content)
}

/** `.row`: строка списка — 14 средний и подпись 12; справа — переключатель, значение или стрелка. */
@Composable
fun KRow(
    title: String, sub: String? = null, wrap: Boolean = false, chevron: Boolean = false, onClick: (() -> Unit)? = null,
    trailing: @Composable RowScope.() -> Unit = {},
) {
    val base = Modifier.fillMaxWidth()
    val m = if (onClick != null) base.clickable(onClick = onClick) else base
    Row(m.padding(horizontal = 14.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            KText(title, weight = FontWeight.Medium, lines = if (wrap) Int.MAX_VALUE else 1)
            if (!sub.isNullOrEmpty()) KText(sub, size = 12.sp, color = K.dim, lines = if (wrap) Int.MAX_VALUE else 1, lineHeight = if (wrap) 16.sp else TextUnit.Unspecified)
        }
        trailing()
        if (chevron) KIcon(KIcons.chevron, 14.dp, K.dim2)
    }
}

/** Линия между строками карточки. */
@Composable
fun KDivider(inset: Dp = 14.dp) {
    Box(Modifier.padding(start = inset).fillMaxWidth().height(1.dp).background(K.line))
}

/** `.toggle` веб-версии: 44×26, белый кружок. */
@Composable
fun KToggle(on: Boolean, onChange: (Boolean) -> Unit) {
    val track by animateColorAsState(if (on) K.accent else K.raised, label = "toggle")
    val x by animateDpAsState(if (on) 21.dp else 3.dp, spring(dampingRatio = 0.75f), label = "knob")
    Box(
        Modifier.size(44.dp, 26.dp).clip(CircleShape).background(track)
            .press(haptic = HapticFeedbackType.TextHandleMove) { onChange(!on) },
    ) {
        Box(Modifier.offset(x = x, y = 3.dp).size(20.dp).shadow(1.5.dp, CircleShape).background(Color.White, CircleShape))
    }
}

/** `.btn-elem`: 28, капсула, фон `--elem`, 12 средний, значок через 6. */
@Composable
fun ElemButton(text: String, icon: Int? = null, enabled: Boolean = true, onClick: () -> Unit) {
    Row(
        Modifier.height(28.dp).clip(CircleShape).background(K.elem).press(enabled, onClick = onClick).padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        if (icon != null) KIcon(icon, 13.dp, if (enabled) K.text else K.dim)
        KText(text, size = 12.sp, weight = FontWeight.Medium, color = if (enabled) K.text else K.dim, lines = 1)
    }
}

/** `.btn-primary`: во всю ширину, 48, скругление 14, фон — цвет текста; выключенная — `--elem`. */
@Composable
fun PrimaryButton(text: String, enabled: Boolean = true, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Box(
        modifier.fillMaxWidth().height(48.dp).clip(RoundedCornerShape(K.r(14))).background(if (enabled) K.text else K.elem)
            .press(enabled, to = 0.98f, haptic = HapticFeedbackType.LongPress, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { KText(text, size = 15.sp, weight = FontWeight.SemiBold, color = if (enabled) K.win else K.dim) }
}

/** `.btn-card`: во всю ширину, 44, скругление 12, фон карточки. */
@Composable
fun CardButton(text: String, icon: Int? = null, enabled: Boolean = true, weight: FontWeight = FontWeight.Medium, modifier: Modifier = Modifier, onClick: () -> Unit) {
    Row(
        modifier.fillMaxWidth().height(44.dp).clip(RoundedCornerShape(K.r(12))).background(K.card).press(enabled, to = 0.98f, onClick = onClick),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.Center,
    ) {
        if (icon != null) {
            KIcon(icon, 18.dp)
            Spacer(Modifier.size(8.dp))
        }
        KText(text, weight = weight, color = if (enabled) K.text else K.dim)
    }
}

/** `.seg`: переключатель на 2–3 положения на подложке карточки; выбранное — «пилюля». */
@Composable
fun <T> KSeg(value: T, options: List<Pair<T, String>>, height: Dp = 34.dp, size: TextUnit = 13.sp, onChange: (T) -> Unit) {
    val haptics = LocalHapticFeedback.current
    Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(K.r(12))).background(K.card).padding(3.dp), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
        options.forEach { (v, title) ->
            val on = v == value
            val bg by animateColorAsState(if (on) K.raised else Color.Transparent, label = "seg")
            Box(
                Modifier.weight(1f).height(height).clip(RoundedCornerShape(K.r(9))).background(bg)
                    .clickable(remember { MutableInteractionSource() }, null) {
                        if (!on) haptics.performHapticFeedback(HapticFeedbackType.TextHandleMove)
                        onChange(v)
                    },
                contentAlignment = Alignment.Center,
            ) { KText(title, size = size, weight = FontWeight.Medium, color = if (on) K.text else K.dim, lines = 1) }
        }
    }
}

/** `.radio`: кружок выбора 20 с точкой 8. */
@Composable
fun KRadio(on: Boolean) {
    Box(Modifier.size(20.dp).border(2.dp, if (on) K.accent else K.ring, CircleShape), contentAlignment = Alignment.Center) {
        if (on) Box(Modifier.size(8.dp).background(K.accent, CircleShape))
    }
}

/** Карточка с обводкой (поле ввода, плашка), скругление веб-версии. */
@Composable
fun Modifier.kOutline(radius: Int, color: Color, shape: Shape = RoundedCornerShape(K.r(radius))): Modifier = border(1.dp, color, shape)
