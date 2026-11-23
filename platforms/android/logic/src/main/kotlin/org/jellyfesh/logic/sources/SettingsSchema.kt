package org.jellyfesh.logic.sources

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.longOrNull

/**
 * A plugin's settings form, rendered natively from the JSON Schema subset in its manifest
 * (ADR-0004): an object of properties that are strings, numbers, integers, booleans or enums,
 * and with settings schema profile 1.2 (docs/plugins/authoring.md) colours (`format: color`),
 * sliders (`minimum` and `maximum`, `multipleOf` the step), ordered checklists (arrays of
 * choices with `uniqueItems`), `oneOf` choices with titles and `x-jf-group` headings. The values
 * go back to the core as JSON (`SetPluginSettings`); keys the form does not know are kept as they
 * were.
 */
sealed interface SchemaField {
    val key: String
    val title: String
    val description: String?
    val required: Boolean

    /** `x-jf-group`: consecutive fields of the same group share its heading (profile 1.2). */
    val group: String?

    data class Text(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        /** Masked input (`format: password` or `writeOnly`). */
        val secret: Boolean,
        /** `format: uri`: a server address. */
        val url: Boolean,
        val placeholder: String?,
        val minLength: Int?,
        val maxLength: Int?,
        val pattern: String?,
        val default: String?,
        override val group: String? = null,
    ) : SchemaField

    data class Number(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val integer: Boolean,
        val minimum: Double?,
        val maximum: Double?,
        val default: Double?,
        override val group: String? = null,
        /** `multipleOf`: a slider's step. */
        val multipleOf: Double? = null,
    ) : SchemaField {
        /**
         * `minimum` and `maximum` make a slider (profile 1.2), unless the range holds more than
         * [SLIDER_STEPS_MAX] steps (a port number stays a text field).
         */
        val slider: Boolean get() {
            if (minimum == null || maximum == null || maximum <= minimum) return false
            val step = multipleOf ?: if (integer) 1.0 else return true
            return (maximum - minimum) / step <= SLIDER_STEPS_MAX
        }

        companion object {
            const val SLIDER_STEPS_MAX = 1_000
        }
    }

    data class Toggle(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val default: Boolean,
        override val group: String? = null,
    ) : SchemaField

    data class Choice(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val options: List<Option>,
        val default: JsonPrimitive?,
        override val group: String? = null,
    ) : SchemaField {
        data class Option(val value: JsonPrimitive, val label: String)

        /** A few short choices fit a segmented control; more open a list to pick from. */
        val segmented: Boolean get() = options.size <= SEGMENTED_MAX

        companion object {
            const val SEGMENTED_MAX = 3
        }
    }

    /** `format: color`: `#RRGGBB`, shown as a colour well (profile 1.2). */
    data class Colour(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val default: String?,
        override val group: String? = null,
    ) : SchemaField

    /**
     * An ordered checklist (profile 1.2: an array of choices with `uniqueItems`): its value lists
     * the items shown, in their order.
     */
    data class Checklist(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val options: List<Choice.Option>,
        val default: List<String>,
        override val group: String? = null,
    ) : SchemaField
}

/** Why a value is not accepted. */
enum class FieldError { Required, TooShort, TooLong, Pattern, NotANumber, NotAnInteger, BelowMinimum, AboveMaximum, NotAnOption, NotAColour }

/** Consecutive fields under one heading ([title] null: the plugin gave no group). */
data class FieldGroup(val title: String?, val fields: List<SchemaField>)

object SettingsSchema {
    private val json = Json { ignoreUnknownKeys = true }

    /** Whether the plugin has settings of its own to show (its Settings button in Settings › Sources). */
    fun hasFields(schemaJson: String): Boolean = parse(schemaJson).isNotEmpty()

    /** The form's fields, in the schema's order; an unreadable schema gives no fields. */
    fun parse(schemaJson: String): List<SchemaField> {
        val root = runCatching { json.parseToJsonElement(schemaJson).jsonObject }.getOrNull() ?: return emptyList()
        val properties = root["properties"] as? JsonObject ?: return emptyList()
        val required = (root["required"] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull }?.toSet().orEmpty()
        return properties.mapNotNull { (key, node) -> field(key, node as? JsonObject ?: return@mapNotNull null, key in required) }
    }

    private fun field(key: String, node: JsonObject, required: Boolean): SchemaField? {
        val title = node.string("title") ?: key
        val description = node.string("description")
        val group = node.string("x-jf-group")?.takeIf { it.isNotBlank() }
        if (node.string("type") == "array") {
            val items = node["items"] as? JsonObject ?: return null
            val options = options(items) ?: return null
            val default = (node["default"] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull }.orEmpty()
            return SchemaField.Checklist(key, title, description, required, options, default, group)
        }
        options(node)?.let { options ->
            return SchemaField.Choice(key, title, description, required, options, node["default"] as? JsonPrimitive, group)
        }
        return when (node.string("type")) {
            "string" -> if (node.string("format") == "color") {
                SchemaField.Colour(key, title, description, required, node.string("default")?.let(ColourWell::normalise), group)
            } else {
                SchemaField.Text(
                    key, title, description, required,
                    secret = node.string("format") == "password" || node.bool("writeOnly") == true || node.bool("x-secret") == true,
                    url = node.string("format") == "uri" || node.string("format") == "url",
                    placeholder = node.string("x-placeholder") ?: (node["examples"] as? JsonArray)?.firstOrNull()?.let { (it as? JsonPrimitive)?.contentOrNull },
                    minLength = node.long("minLength")?.toInt(),
                    maxLength = node.long("maxLength")?.toInt(),
                    pattern = node.string("pattern"),
                    default = node.string("default"),
                    group = group,
                )
            }
            "number", "integer" -> SchemaField.Number(
                key, title, description, required,
                integer = node.string("type") == "integer",
                minimum = node.double("minimum"),
                maximum = node.double("maximum"),
                default = node.double("default"),
                group = group,
                multipleOf = node.double("multipleOf")?.takeIf { it > 0.0 },
            )
            "boolean" -> SchemaField.Toggle(key, title, description, required, node.bool("default") ?: false, group)
            else -> null
        }
    }

    /** The choices of `enum` (with `x-enum-labels`) or `oneOf` (`const` and `title`); null when neither. */
    private fun options(node: JsonObject): List<SchemaField.Choice.Option>? {
        val enum = node["enum"] as? JsonArray
        val oneOf = node["oneOf"] as? JsonArray
        if (enum == null && oneOf == null) return null
        val labels = (node["x-enum-labels"] as? JsonArray)?.map { (it as? JsonPrimitive)?.contentOrNull }
        return enum?.mapIndexedNotNull { i, v ->
            val p = v as? JsonPrimitive ?: return@mapIndexedNotNull null
            SchemaField.Choice.Option(p, labels?.getOrNull(i) ?: p.contentOrNull ?: p.toString())
        } ?: oneOf!!.mapNotNull { option ->
            val o = option as? JsonObject ?: return@mapNotNull null
            val value = o["const"] as? JsonPrimitive ?: return@mapNotNull null
            SchemaField.Choice.Option(value, o.string("title") ?: value.contentOrNull ?: value.toString())
        }
    }

    /** The fields under their headings: consecutive fields of the same `x-jf-group` together. */
    fun groups(fields: List<SchemaField>): List<FieldGroup> {
        val out = ArrayList<FieldGroup>()
        for (f in fields) {
            val last = out.lastOrNull()
            if (last != null && last.title == f.group) {
                out[out.size - 1] = last.copy(fields = last.fields + f)
            } else {
                out += FieldGroup(f.group, listOf(f))
            }
        }
        return out
    }

    /**
     * Whether the form should come from `localized_settings_schema` for the app's language (a
     * BCP 47 tag): every language but English, whose words are the schema's own.
     */
    fun wantsTranslation(languageTag: String): Boolean {
        val language = languageTag.trim().replace('_', '-').substringBefore('-').lowercase()
        return language.isNotEmpty() && language != "en" && language != "und"
    }

    /** A checklist's value as the form keeps it (a JSON array of the shown items, in order). */
    fun checklist(text: String?): List<String> =
        runCatching { (json.parseToJsonElement(text.orEmpty()) as JsonArray).mapNotNull { (it as? JsonPrimitive)?.contentOrNull } }.getOrDefault(emptyList())

    /** The text the form keeps for a checklist's [items]. */
    fun checklistText(items: List<String>): String = JsonArray(items.map(::JsonPrimitive)).toString()

    /** A slider's value moved to [raw]: within its range, on its step (counted from its minimum). */
    fun snap(field: SchemaField.Number, raw: Double): Double {
        val min = field.minimum ?: Double.NEGATIVE_INFINITY
        val max = field.maximum ?: Double.POSITIVE_INFINITY
        var v = raw.coerceIn(min, max)
        val step = field.multipleOf ?: if (field.integer) 1.0 else null
        if (step != null && min.isFinite()) {
            v = (min + Math.round((v - min) / step) * step).coerceIn(min, max)
        }
        if (field.integer) v = Math.round(v).toDouble()
        // Ten decimals at most: 0.1 + 0.2 shows as 0.3.
        return Math.round(v * 1e10) / 1e10
    }

    /** A number as the form keeps and shows it: `1`, `0.75`, `1.05`. */
    fun numberText(value: Double, integer: Boolean = false): String =
        if (integer || value == Math.floor(value) && !value.isInfinite()) {
            value.toLong().toString()
        } else {
            java.math.BigDecimal(value).setScale(6, java.math.RoundingMode.HALF_UP).stripTrailingZeros().toPlainString()
        }

    /** The current values as the form edits them (text), from the plugin's settings JSON. */
    fun values(fields: List<SchemaField>, settingsJson: String): Map<String, String> {
        val current = runCatching { json.parseToJsonElement(settingsJson).jsonObject }.getOrNull() ?: JsonObject(emptyMap())
        return fields.associate { f ->
            val v = current[f.key] as? JsonPrimitive
            f.key to when (f) {
                is SchemaField.Text -> v?.contentOrNull ?: f.default.orEmpty()
                is SchemaField.Number -> v?.contentOrNull ?: f.default?.let { format(it, f.integer) }.orEmpty()
                is SchemaField.Toggle -> (v?.booleanOrNull ?: f.default).toString()
                is SchemaField.Choice -> (v ?: f.default)?.contentOrNull.orEmpty()
                is SchemaField.Colour -> v?.contentOrNull?.let(ColourWell::normalise) ?: f.default.orEmpty()
                is SchemaField.Checklist -> checklistText(
                    (current[f.key] as? JsonArray)?.mapNotNull { (it as? JsonPrimitive)?.contentOrNull } ?: f.default,
                )
            }
        }
    }

    /** Problems with [values]; empty when the form can be saved. */
    fun validate(fields: List<SchemaField>, values: Map<String, String>): Map<String, FieldError> {
        val errors = LinkedHashMap<String, FieldError>()
        for (f in fields) {
            val text = values[f.key].orEmpty()
            if (text.isEmpty()) {
                if (f.required && f !is SchemaField.Toggle && f !is SchemaField.Checklist) errors[f.key] = FieldError.Required
                continue
            }
            val error: FieldError? = when (f) {
                is SchemaField.Text -> when {
                    f.minLength != null && text.length < f.minLength -> FieldError.TooShort
                    f.maxLength != null && text.length > f.maxLength -> FieldError.TooLong
                    f.pattern != null && runCatching { !Regex(f.pattern).containsMatchIn(text) }.getOrDefault(false) -> FieldError.Pattern
                    else -> null
                }
                is SchemaField.Number -> {
                    val n = text.toDoubleOrNull()
                    when {
                        n == null -> FieldError.NotANumber
                        f.integer && n != Math.floor(n) -> FieldError.NotAnInteger
                        f.minimum != null && n < f.minimum -> FieldError.BelowMinimum
                        f.maximum != null && n > f.maximum -> FieldError.AboveMaximum
                        else -> null
                    }
                }
                is SchemaField.Toggle -> null
                is SchemaField.Choice -> if (f.options.none { it.value.contentOrNull == text }) FieldError.NotAnOption else null
                is SchemaField.Colour -> if (ColourWell.parse(text) == null) FieldError.NotAColour else null
                is SchemaField.Checklist -> {
                    val items = checklist(text)
                    val known = f.options.mapNotNull { it.value.contentOrNull }.toSet()
                    if (items.any { it !in known } || items.toSet().size != items.size) FieldError.NotAnOption else null
                }
            }
            if (error != null) errors[f.key] = error
        }
        return errors
    }

    /** The settings JSON to send: [original] with the form's values written over it. */
    fun toJson(fields: List<SchemaField>, values: Map<String, String>, original: String): String {
        val base = runCatching { json.parseToJsonElement(original).jsonObject }.getOrNull() ?: JsonObject(emptyMap())
        val out = LinkedHashMap<String, JsonElement>(base)
        for (f in fields) {
            val text = values[f.key] ?: continue
            out[f.key] = when (f) {
                is SchemaField.Text -> if (text.isEmpty() && !f.required) JsonNull else JsonPrimitive(text)
                is SchemaField.Number -> text.toDoubleOrNull()?.let { if (f.integer) JsonPrimitive(it.toLong()) else JsonPrimitive(it) } ?: JsonNull
                is SchemaField.Toggle -> JsonPrimitive(text.toBoolean())
                is SchemaField.Choice -> f.options.firstOrNull { it.value.contentOrNull == text }?.value ?: JsonNull
                is SchemaField.Colour -> ColourWell.normalise(text)?.let(::JsonPrimitive) ?: JsonNull
                is SchemaField.Checklist -> JsonArray(checklist(text).map(::JsonPrimitive))
            }
        }
        return JsonObject(out).toString()
    }

    private fun format(value: Double, integer: Boolean): String =
        if (integer || value == Math.floor(value)) value.toLong().toString() else value.toString()

    private fun JsonObject.string(name: String): String? = (this[name] as? JsonPrimitive)?.takeIf { it.isString }?.content
    private fun JsonObject.bool(name: String): Boolean? = (this[name] as? JsonPrimitive)?.booleanOrNull
    private fun JsonObject.long(name: String): Long? = (this[name] as? JsonPrimitive)?.longOrNull
    private fun JsonObject.double(name: String): Double? = (this[name] as? JsonPrimitive)?.doubleOrNull
}
