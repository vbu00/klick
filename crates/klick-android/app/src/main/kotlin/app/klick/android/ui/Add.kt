package app.klick.android.ui

import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
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
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.klick.android.KlickModel
import app.klick.android.Texts
import app.klick.android.Toast
import kotlinx.coroutines.launch

/** «Добавить подключение» — как ui/src/screens/Add.tsx и iOS β: по ссылке и из файла. */
@Composable
fun AddScreen(m: KlickModel, incoming: String?, onIncomingUsed: () -> Unit, onTab: (Int) -> Unit) {
    var mode by rememberSaveable { mutableStateOf("link") }
    var link by rememberSaveable { mutableStateOf("") }
    var name by rememberSaveable { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current

    LaunchedEffect(incoming) {
        if (incoming != null) {
            link = Texts.fromDeepLink(incoming)
            mode = "link"
            onIncomingUsed()
        }
    }

    val d = Texts.detect(link)
    val canAdd = d.kind in setOf("sub", "link", "text") && !busy

    fun addLink() {
        if (!canAdd) return
        busy = true
        val text = link.trim()
        val title = name.trim()
        scope.launch {
            val ok = if (d.kind == "text") m.importText(title.ifEmpty { d.name ?: "Серверы" }, text) else m.add(text, if (d.kind == "sub") title else "")
            busy = false
            if (ok) {
                link = ""
                name = ""
                onTab(TAB_HOME)
            }
        }
    }

    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
        uri ?: return@rememberLauncherForActivityResult
        busy = true
        scope.launch {
            val fileName = ctx.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0) else null
            } ?: "config.yaml"
            val content = runCatching { ctx.contentResolver.openInputStream(uri)?.use { it.readBytes().decodeToString() } }.getOrNull()
            if (content == null) m.toast = Toast("Не удалось прочитать файл", fileName, "bad")
            else if (m.importFile(fileName, content)) onTab(TAB_HOME)
            busy = false
        }
    }

    KScreen {
        KTitle("Добавить подключение")
        Box(Modifier.padding(top = 18.dp)) { KSeg(mode, listOf("link" to "По ссылке", "file" to "Из файла")) { mode = it } }
        if (mode == "link") {
            ByLink(link, { link = it }, name, { name = it }, d, busy, canAdd, onAdd = ::addLink)
        } else {
            FromFile(busy) { picker.launch(arrayOf("*/*")) }
        }
    }
}

@Composable
private fun ByLink(
    link: String, onLink: (String) -> Unit, name: String, onName: (String) -> Unit, d: Texts.Detect, busy: Boolean, canAdd: Boolean, onAdd: () -> Unit,
) {
    val clipboard = LocalClipboardManager.current
    CardButton("Вставить из буфера", KIcons.copy, enabled = !busy, modifier = Modifier.padding(top = 12.dp)) {
        clipboard.getText()?.text?.trim()?.takeIf { it.isNotEmpty() }?.let { onLink(Texts.fromDeepLink(it)) }
    }

    val border = when (d.kind) {
        "bad" -> K.red.copy(alpha = 0.55f)
        "empty" -> K.line
        else -> K.accent.copy(alpha = 0.55f)
    }
    Box(
        Modifier.padding(top = 16.dp).fillMaxWidth().height(110.dp).clip(RoundedCornerShape(K.r(14))).background(K.card)
            .border(1.dp, border, RoundedCornerShape(K.r(14))).padding(14.dp),
    ) {
        if (link.isEmpty()) KText("https://panel.example.com/sub/a1b2… или vless://…", size = 13.sp, color = K.dim2, mono = true)
        BasicTextField(
            link, onLink, Modifier.fillMaxWidth(),
            textStyle = TextStyle(fontSize = 13.sp, color = K.text, fontFamily = FontFamily.Monospace),
            cursorBrush = SolidColor(K.accent),
            keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, keyboardType = KeyboardType.Uri),
        )
    }

    Row(
        Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(12))).kCard(12).padding(horizontal = 14.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Box(Modifier.size(8.dp).background(when (d.tone) { "accent" -> K.accent; "red" -> K.red; else -> K.dim }, CircleShape))
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            KText(d.title, size = 13.sp, weight = FontWeight.SemiBold)
            KText(d.text, size = 12.sp, color = K.text3)
        }
    }

    if (d.kind == "sub" || d.kind == "text") {
        Box(Modifier.padding(top = 10.dp).fillMaxWidth().height(44.dp).clip(RoundedCornerShape(K.r(12))).kCard(12).padding(horizontal = 14.dp), contentAlignment = Alignment.CenterStart) {
            if (name.isEmpty()) KText(if (d.kind == "text") "Название (иначе «${d.name.orEmpty()}»)" else "Название (необязательно)", color = K.dim2)
            BasicTextField(
                name, onName, Modifier.fillMaxWidth(), singleLine = true, textStyle = TextStyle(fontSize = 14.sp, color = K.text),
                cursorBrush = SolidColor(K.accent), keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            )
        }
    }

    PrimaryButton(if (busy) "Добавляем…" else "Добавить", enabled = canAdd, modifier = Modifier.padding(top = 14.dp), onClick = onAdd)

    KCaps("Что можно вставить", Modifier.padding(top = 26.dp))
    Column(Modifier.padding(top = 10.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(20))).kCard()) {
        Help(
            KIcons.globe, "Ссылка на подписку",
            "https://… от Remnawave, Marzban и подобных — в форматах Clash, sing-box, Xray или списком ссылок; ключ Outline ssconf://. Даёт список серверов, лимит трафика и срок действия. Обновляется автоматически.",
        )
        Box(Modifier.fillMaxWidth().height(1.dp).background(K.line))
        Help(KIcons.rules, "Одиночная конфигурация", "vless://, vmess://, trojan://, ss://, hysteria2:// — прямое соединение с одним сервером.")
    }
}

@Composable
private fun Help(icon: Int, title: String, text: String) {
    Row(Modifier.padding(horizontal = 14.dp, vertical = 12.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
        Box(Modifier.size(32.dp).clip(RoundedCornerShape(K.r(9))).background(K.elem), contentAlignment = Alignment.Center) { KIcon(icon, 18.dp, K.dim) }
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            KText(title, size = 13.sp, weight = FontWeight.SemiBold)
            KText(text, size = 12.sp, color = K.text3, lineHeight = 16.sp)
        }
    }
}

@Composable
private fun FromFile(busy: Boolean, onPick: () -> Unit) {
    val raised = K.raised
    Column(
        Modifier.padding(top = 16.dp).fillMaxWidth().height(190.dp).clip(RoundedCornerShape(K.r(16))).background(K.card.copy(alpha = 0.5f))
            .drawBehind {
                drawRoundRect(raised, cornerRadius = CornerRadius(16.dp.toPx()), style = Stroke(1.5.dp.toPx(), pathEffect = PathEffect.dashPathEffect(floatArrayOf(6.dp.toPx(), 4.dp.toPx()))))
            }
            .clickable(enabled = !busy, onClick = onPick),
        horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp, Alignment.CenterVertically),
    ) {
        KIcon(KIcons.upload, 34.dp, K.dim)
        KText(if (busy) "Импортируем…" else "Выберите файл", weight = FontWeight.SemiBold)
        KText("из «Файлов» или «Загрузок»", size = 12.sp, color = K.dim)
    }
    KText(
        "Поддерживаются конфиги Mihomo / Clash (.yaml, .json), sing-box и Xray (.json), WireGuard и AmneziaWG (.conf), Shadowsocks и Outline (.json), списки ссылок (.txt). Из файла берутся серверы; файл копируется в приложение — оригинал можно удалить.",
        size = 12.sp, color = K.text3, lineHeight = 17.sp,
        modifier = Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(K.r(14))).kCard(14).padding(14.dp),
    )
}
