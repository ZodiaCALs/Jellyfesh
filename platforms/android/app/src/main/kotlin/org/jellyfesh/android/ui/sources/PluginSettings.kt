package org.jellyfesh.android.ui.sources

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.serialization.json.contentOrNull
import org.jellyfesh.android.R
import org.jellyfesh.android.ui.components.Divider
import org.jellyfesh.android.ui.components.JfText
import org.jellyfesh.android.ui.components.LocalOverlays
import org.jellyfesh.android.ui.components.Notice
import org.jellyfesh.android.ui.components.Panel
import org.jellyfesh.android.ui.components.SectionHeader
import org.jellyfesh.android.ui.components.SettingRow
import org.jellyfesh.android.ui.components.bleed
import org.jellyfesh.android.ui.data.LocalGraph
import org.jellyfesh.android.ui.theme.ButtonStyle
import org.jellyfesh.android.ui.theme.Jf
import org.jellyfesh.android.ui.theme.JfButton
import org.jellyfesh.android.ui.theme.JfSwitch
import org.jellyfesh.android.ui.theme.JfTextField
import org.jellyfesh.android.ui.theme.JfType
import org.jellyfesh.android.ui.theme.SegmentedControl
import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.PluginSummary
import org.jellyfesh.logic.sources.FieldError
import org.jellyfesh.logic.sources.SchemaField
import org.jellyfesh.logic.sources.SettingsSchema

/**
 * A plugin's settings, drawn natively from the JSON Schema in its manifest (ADR-0004) and sent
 * back as JSON with `SetPluginSettings`. Keys the form does not know are kept as they were.
 */
@Composable
fun PluginSettingsPanel(plugin: PluginSummary) {
    val graph = LocalGraph.current
    val overlays = LocalOverlays.current
    val fields = remember(plugin.settingsSchemaJson) { SettingsSchema.parse(plugin.settingsSchemaJson) }
    if (fields.isEmpty()) {
        if (plugin.settingsSchemaJson.isNotBlank() && plugin.settingsSchemaJson.trim() != "{}") {
            Notice(stringResource(R.string.source_settings_unreadable), warning = true)
        }
        return
    }
    var values by remember(plugin.id, plugin.settingsJson) { mutableStateOf(SettingsSchema.values(fields, plugin.settingsJson)) }
    var showErrors by remember(plugin.id) { mutableStateOf(false) }
    val errors = SettingsSchema.validate(fields, values)
    val saved = stringResource(R.string.settings_saved)
    SectionHeader(stringResource(R.string.source_settings))
    Panel {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            fields.forEachIndexed { i, field ->
                if (i > 0 && field is SchemaField.Toggle) Divider(Modifier.padding(end = 0.dp))
                val error = errors[field.key]?.takeIf { showErrors }?.let { fieldError(it, field) }
                when (field) {
                    is SchemaField.Text -> JfTextField(
                        value = values[field.key].orEmpty(),
                        onValueChange = { values = values + (field.key to it) },
                        label = field.title,
                        placeholder = field.placeholder,
                        secret = field.secret,
                        keyboard = if (field.url) KeyboardType.Uri else KeyboardType.Text,
                        error = error,
                        supporting = field.description,
                    )
                    is SchemaField.Number -> JfTextField(
                        value = values[field.key].orEmpty(),
                        onValueChange = { values = values + (field.key to it) },
                        label = field.title,
                        keyboard = if (field.integer) KeyboardType.Number else KeyboardType.Decimal,
                        error = error,
                        supporting = field.description,
                    )
                    is SchemaField.Toggle -> SettingRow(field.title, Modifier.bleed(16.dp), description = field.description) {
                        JfSwitch(values[field.key].toBoolean(), { values = values + (field.key to it.toString()) })
                    }
                    is SchemaField.Choice -> Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        JfText(field.title, style = JfType.label, color = Jf.colors.ink70)
                        SegmentedControl(
                            options = field.options,
                            selected = field.options.firstOrNull { it.value.contentOrNull == values[field.key] },
                            label = { it.label },
                            onSelect = { values = values + (field.key to (it.value.contentOrNull ?: "")) },
                            description = field.title,
                        )
                        field.description?.let { JfText(it, style = JfType.caption, color = Jf.colors.ink50) }
                        if (error != null) JfText(error, style = JfType.caption, color = Jf.colors.danger)
                    }
                }
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
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
}

private fun number(value: Double?): String = when {
    value == null -> ""
    value == Math.floor(value) -> value.toLong().toString()
    else -> value.toString()
}
