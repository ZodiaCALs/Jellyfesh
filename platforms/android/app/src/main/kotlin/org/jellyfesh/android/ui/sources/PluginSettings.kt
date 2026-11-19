package org.jellyfesh.android.ui.sources

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import kotlinx.serialization.json.contentOrNull
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.ChoiceRow
import org.jellyfesh.android.ui.components.JfIcon
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.components.bleed
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.data.rememberQuery
import org.jellyfesh.android.ui.library.DragHandle
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfSlider
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.android.valueOrNull
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.sources.Checklists
import org.jellyfesh.logic.sources.ColourWell
import org.jellyfesh.logic.sources.FieldError
import org.jellyfesh.logic.sources.SchemaField
import org.jellyfesh.logic.sources.SettingsSchema

/**
 * A plugin's settings, drawn natively from the JSON Schema in its manifest (ADR-0004; settings
 * schema profile 1.2) and sent back as JSON with `SetPluginSettings` when the person saves (an
 * allowed difference: Save on Android). In a language other than English the form comes from the
 * core's `localized_settings_schema`, the plugin's own translations. Fields of a group sit under
 * its heading. Keys the form does not know are kept as they were. [resetToDefaults] adds Reset to
 * Defaults (`ResetPluginSettings`), as Customize has.
 */
@Composable
fun PluginSettingsPanel(plugin: PluginSummary, resetToDefaults: Boolean = false) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val language = LocalConfiguration.current.locales[0].toLanguageTag()
    val translated by rememberQuery(plugin.id, plugin.settingsSchemaJson, language) { core ->
        if (SettingsSchema.wantsTranslation(language)) core.localizedSettingsSchema(plugin.id, language) else null
    }
    val schemaText = translated.valueOrNull() ?: plugin.settingsSchemaJson
    val fields = remember(schemaText) { SettingsSchema.parse(schemaText) }
    if (fields.isEmpty()) {
        if (plugin.settingsSchemaJson.isNotBlank() && plugin.settingsSchemaJson.trim() != "{}") {
            Notice(stringResource(R.string.source_settings_unreadable), warning = true)
        }
        return
    }
    // The translated schema has the same keys and defaults, so the values outlive the switch to it.
    var values by remember(plugin.id, plugin.settingsJson) { mutableStateOf(SettingsSchema.values(fields, plugin.settingsJson)) }
    var showErrors by remember(plugin.id) { mutableStateOf(false) }
    val errors = SettingsSchema.validate(fields, values)
    val saved = stringResource(R.string.settings_saved)
    val settingsTitle = stringResource(R.string.source_settings)
    Column {
        SettingsSchema.groups(fields).forEach { group ->
            SectionHeader(group.title ?: settingsTitle)
            Panel {
                Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    group.fields.forEachIndexed { i, field ->
                        if (i > 0 && field is SchemaField.Toggle) Divider(Modifier.padding(end = 0.dp))
                        val error = errors[field.key]?.takeIf { showErrors }?.let { fieldError(it, field) }
                        FieldControl(field, values[field.key].orEmpty(), error) { values = values + (field.key to it) }
                    }
                }
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        ) {
            if (resetToDefaults) {
                JfButton(stringResource(R.string.action_reset_defaults), { graph.send(Command.ResetPluginSettings(plugin.id)) }, style = ButtonStyle.Quiet)
            }
            JfButton(stringResource(R.string.action_save), {
                if (errors.isNotEmpty()) {
                    showErrors = true
                } else {
                    graph.send(Command.SetPluginSettings(plugin.id, SettingsSchema.toJson(fields, values, plugin.settingsJson)))
                    overlays.toast(saved)
                }
            }, style = ButtonStyle.Accent)
        }
    }
}

/** One field of the form, as its kind is drawn. */
@Composable
private fun FieldControl(field: SchemaField, value: String, error: String?, onChange: (String) -> Unit) {
    when (field) {
        is SchemaField.Text -> JfTextField(
            value = value,
            onValueChange = onChange,
            label = field.title,
            placeholder = field.placeholder,
            secret = field.secret,
            keyboard = if (field.url) KeyboardType.Uri else KeyboardType.Text,
            error = error,
            supporting = field.description,
        )
        is SchemaField.Number -> if (field.slider) {
            SliderField(field, value, onChange)
        } else {
            JfTextField(
                value = value,
                onValueChange = onChange,
                label = field.title,
                keyboard = if (field.integer) KeyboardType.Number else KeyboardType.Decimal,
                error = error,
                supporting = field.description,
            )
        }
        is SchemaField.Toggle -> SettingRow(field.title, Modifier.bleed(16.dp), description = field.description) {
            JfSwitch(value.toBoolean(), { onChange(it.toString()) })
        }
        is SchemaField.Choice -> if (field.segmented) {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                JfText(field.title, style = JfType.label, color = Jf.colors.ink70)
                SegmentedControl(
                    options = field.options,
                    selected = field.options.firstOrNull { it.value.contentOrNull == value },
                    label = { it.label },
                    onSelect = { onChange(it.value.contentOrNull ?: "") },
                    description = field.title,
                )
                FieldNotes(field.description, error)
            }
        } else {
            ChoiceListField(field, value, error, onChange)
        }
        is SchemaField.Colour -> ColourField(field, value, error, onChange)
        is SchemaField.Checklist -> ChecklistField(field, value, error, onChange)
    }
}

@Composable
private fun FieldNotes(description: String?, error: String?) {
    description?.let { JfText(it, style = JfType.caption, color = Jf.colors.ink50) }
    if (error != null) JfText(error, style = JfType.caption, color = Jf.colors.danger)
}

/** A number with `minimum` and `maximum`: a slider on its `multipleOf` steps, with its value beside the title. */
@Composable
private fun SliderField(field: SchemaField.Number, value: String, onChange: (String) -> Unit) {
    val min = field.minimum ?: 0.0
    val max = field.maximum ?: 1.0
    val current = (value.toDoubleOrNull() ?: field.default ?: min).coerceIn(min, max)
    val text = SettingsSchema.numberText(current, field.integer)
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            JfText(field.title, style = JfType.label, color = Jf.colors.ink70, modifier = Modifier.weight(1f))
            JfText(text, style = JfType.label, color = Jf.colors.ink90)
        }
        JfSlider(
            value = current.toFloat(),
            range = min.toFloat()..max.toFloat(),
            onChange = { onChange(SettingsSchema.numberText(SettingsSchema.snap(field, it.toDouble()), field.integer)) },
            description = field.title,
            valueText = text,
            step = (field.multipleOf ?: if (field.integer) 1.0 else 0.0).toFloat(),
        )
        FieldNotes(field.description, null)
    }
}

/** A choice with more options than a segmented control holds: its value, and a list to pick from. */
@Composable
private fun ChoiceListField(field: SchemaField.Choice, value: String, error: String?, onChange: (String) -> Unit) {
    val overlays = LocalOverlays.current
    val chosen = field.options.firstOrNull { it.value.contentOrNull == value }
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(
            Modifier
                .fillMaxWidth()
                .bleed(16.dp)
                .heightIn(min = 52.dp)
                .clickable(role = Role.Button) {
                    overlays.showSheet(field.title) { dismiss ->
                        Column(Modifier.padding(top = 4.dp)) {
                            field.options.forEach { option ->
                                val v = option.value.contentOrNull ?: ""
                                ChoiceRow(option.label, v == value, {
                                    onChange(v)
                                    dismiss()
                                })
                            }
                        }
                    }
                }
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            JfText(field.title, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize), modifier = Modifier.weight(1f))
            JfText(chosen?.label ?: value, style = JfType.small, color = Jf.colors.ink70, maxLines = 1)
            JfIcon(R.drawable.ic_chevron_right, Jf.colors.ink50, size = 18.dp)
        }
        FieldNotes(field.description, error)
    }
}

/** `format: color`: a colour well; choosing it opens the picker. */
@Composable
private fun ColourField(field: SchemaField.Colour, value: String, error: String?, onChange: (String) -> Unit) {
    val overlays = LocalOverlays.current
    val rgb = ColourWell.parse(value)
    val shown = rgb?.let { ColourWell.hex(it) } ?: value
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Row(
            Modifier
                .fillMaxWidth()
                .bleed(16.dp)
                .heightIn(min = 52.dp)
                .clickable(role = Role.Button) {
                    overlays.showSheet(field.title) { dismiss ->
                        ColourPicker(rgb ?: field.default?.let(ColourWell::parse) ?: 0x808080, dismiss) { onChange(ColourWell.hex(it)) }
                    }
                }
                .semantics { stateDescription = shown }
                .padding(horizontal = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            JfText(field.title, style = JfType.listTitle.copy(fontSize = JfType.body.fontSize), modifier = Modifier.weight(1f))
            JfText(shown, style = JfType.small, color = Jf.colors.ink70, maxLines = 1)
            ColourDot(rgb)
        }
        FieldNotes(field.description, error)
    }
}

@Composable
private fun ColourDot(rgb: Int?, size: Int = 28) {
    Box(
        Modifier
            .size(size.dp)
            .clip(CircleShape)
            .background(if (rgb != null) Color(0xFF000000.toInt() or rgb) else Color.Transparent)
            .border(2.dp, Jf.colors.glassEdge, CircleShape),
    )
}

/** The colour well's picker: hue, saturation and brightness, or the code typed; Done keeps it. */
@Composable
private fun ColourPicker(initial: Int, dismiss: () -> Unit, onDone: (Int) -> Unit) {
    var hsv by remember { mutableStateOf(ColourWell.hsv(initial)) }
    var code by remember { mutableStateOf(ColourWell.hex(initial)) }
    val rgb = ColourWell.rgb(hsv)
    fun set(next: ColourWell.Hsv) {
        hsv = next
        code = ColourWell.hex(ColourWell.rgb(next))
    }
    fun colour(h: Float, s: Float, v: Float) = Color(0xFF000000.toInt() or ColourWell.rgb(ColourWell.Hsv(h, s, v)))
    Column(Modifier.padding(horizontal = 20.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            ColourDot(rgb, size = 44)
            JfTextField(
                value = code,
                onValueChange = { typed ->
                    code = typed
                    ColourWell.parse(typed)?.let { hsv = ColourWell.hsv(it) }
                },
                label = stringResource(R.string.colour_code),
                placeholder = "#RRGGBB",
                modifier = Modifier.weight(1f),
            )
        }
        val hueLabel = stringResource(R.string.colour_hue)
        JfText(hueLabel, style = JfType.label, color = Jf.colors.ink70)
        JfSlider(
            value = hsv.hue,
            range = 0f..359f,
            step = 1f,
            onChange = { set(hsv.copy(hue = it)) },
            description = hueLabel,
            valueText = stringResource(R.string.hue_value, hsv.hue.toInt()),
            track = Brush.horizontalGradient((0..6).map { colour(it * 60f, 1f, 1f) }),
        )
        val saturationLabel = stringResource(R.string.colour_saturation)
        JfText(saturationLabel, style = JfType.label, color = Jf.colors.ink70)
        JfSlider(
            value = hsv.saturation,
            range = 0f..1f,
            step = 0.01f,
            onChange = { set(hsv.copy(saturation = it)) },
            description = saturationLabel,
            valueText = stringResource(R.string.transparency_value, (hsv.saturation * 100).toInt()),
            track = Brush.horizontalGradient(listOf(colour(hsv.hue, 0f, hsv.value), colour(hsv.hue, 1f, hsv.value))),
        )
        val brightnessLabel = stringResource(R.string.colour_brightness)
        JfText(brightnessLabel, style = JfType.label, color = Jf.colors.ink70)
        JfSlider(
            value = hsv.value,
            range = 0f..1f,
            step = 0.01f,
            onChange = { set(hsv.copy(value = it)) },
            description = brightnessLabel,
            valueText = stringResource(R.string.transparency_value, (hsv.value * 100).toInt()),
            track = Brush.horizontalGradient(listOf(colour(hsv.hue, hsv.saturation, 0f), colour(hsv.hue, hsv.saturation, 1f))),
        )
        Row(Modifier.fillMaxWidth().padding(top = 4.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End)) {
            JfButton(stringResource(R.string.action_cancel), dismiss, style = ButtonStyle.Quiet)
            JfButton(stringResource(R.string.action_done), {
                onDone(rgb)
                dismiss()
            }, style = ButtonStyle.Accent)
        }
    }
}

/** An ordered checklist: the shown items first, each with a check and a handle to drag it, then the others. */
@Composable
private fun ChecklistField(field: SchemaField.Checklist, value: String, error: String?, onChange: (String) -> Unit) {
    val shown = SettingsSchema.checklist(value)
    val rows = Checklists.rows(field, shown)
    val rowHeight = 52.dp
    val rowPx = with(LocalDensity.current) { rowHeight.toPx() }
    var drag by remember(field.key) { mutableStateOf<Checklists.Drag?>(null) }
    val moveUp = stringResource(R.string.a11y_move_up)
    val moveDown = stringResource(R.string.a11y_move_down)
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        JfText(field.title, style = JfType.label, color = Jf.colors.ink70)
        Column(Modifier.bleed(16.dp)) {
            rows.forEachIndexed { i, row ->
                val held = row.shown && drag?.from == i
                Row(
                    Modifier
                        .fillMaxWidth()
                        .height(rowHeight)
                        .zIndex(if (held) 1f else 0f)
                        .graphicsLayer {
                            if (row.shown) translationY = Checklists.shift(i, drag, rowPx, shown.size)
                            if (held) shadowElevation = 12f
                        }
                        .background(if (held) Jf.colors.cloud else Color.Transparent)
                        .semantics {
                            if (row.shown) {
                                customActions = listOfNotNull(
                                    CustomAccessibilityAction(moveUp) {
                                        onChange(SettingsSchema.checklistText(Checklists.move(shown, i, i - 1)))
                                        true
                                    }.takeIf { i > 0 },
                                    CustomAccessibilityAction(moveDown) {
                                        onChange(SettingsSchema.checklistText(Checklists.move(shown, i, i + 1)))
                                        true
                                    }.takeIf { i < shown.size - 1 },
                                )
                            }
                        }
                        .padding(start = 16.dp, end = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Row(
                        Modifier
                            .weight(1f)
                            .height(rowHeight)
                            .toggleable(row.shown, role = Role.Checkbox) { on ->
                                onChange(SettingsSchema.checklistText(Checklists.toggle(shown, row.value, on)))
                            },
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                    ) {
                        CheckMark(row.shown)
                        JfText(
                            row.label,
                            style = JfType.listTitle.copy(fontSize = JfType.body.fontSize),
                            color = if (row.shown) Jf.colors.ink90 else Jf.colors.ink50,
                            maxLines = 1,
                        )
                    }
                    if (row.shown) {
                        DragHandle(
                            key = row.value.hashCode().toLong(),
                            onStart = { drag = Checklists.Drag(i, 0f) },
                            onDrag = { dy -> drag = drag?.let { it.copy(offsetPx = it.offsetPx + dy) } },
                            onEnd = {
                                val let = drag
                                drag = null
                                if (let != null) {
                                    val moved = Checklists.drop(shown, let, rowPx)
                                    if (moved != shown) onChange(SettingsSchema.checklistText(moved))
                                }
                            },
                        )
                    } else {
                        Box(Modifier.size(48.dp))
                    }
                }
            }
        }
        FieldNotes(field.description, error)
    }
}

/** A round check: filled with a tick when the item is shown. */
@Composable
private fun CheckMark(on: Boolean) {
    val colors = Jf.colors
    Box(
        Modifier
            .size(24.dp)
            .clip(CircleShape)
            .background(if (on) colors.accent else Color.Transparent)
            .border(if (on) 0.dp else 1.5.dp, colors.lineStrong, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        if (on) JfIcon(R.drawable.ic_check, colors.onAccent, size = 16.dp)
    }
}

@Composable
private fun fieldError(error: FieldError, field: SchemaField): String = when (error) {
    FieldError.Required -> stringResource(R.string.field_required)
    FieldError.TooShort -> stringResource(R.string.field_too_short)
    FieldError.TooLong -> stringResource(R.string.field_too_long)
    FieldError.Pattern -> stringResource(R.string.field_pattern)
    FieldError.NotANumber -> stringResource(R.string.field_not_number)
    FieldError.NotAnInteger -> stringResource(R.string.field_not_integer)
    FieldError.BelowMinimum -> stringResource(R.string.field_below_min, number((field as? SchemaField.Number)?.minimum))
    FieldError.AboveMaximum -> stringResource(R.string.field_above_max, number((field as? SchemaField.Number)?.maximum))
    FieldError.NotAnOption -> stringResource(R.string.field_not_option)
    FieldError.NotAColour -> stringResource(R.string.field_not_colour)
}

private fun number(value: Double?): String = when {
    value == null -> ""
    value == Math.floor(value) -> value.toLong().toString()
    else -> value.toString()
}
