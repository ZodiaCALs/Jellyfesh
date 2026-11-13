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
 * (ADR-0004): an object of properties that are strings, numbers, integers, booleans or enums.
 * The values go back to the core as JSON (`SetPluginSettings`); keys the form does not know are
 * kept as they were.
 */
sealed interface SchemaField {
    val key: String
    val title: String
    val description: String?
    val required: Boolean

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
    ) : SchemaField

    data class Toggle(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val default: Boolean,
    ) : SchemaField

    data class Choice(
        override val key: String,
        override val title: String,
        override val description: String?,
        override val required: Boolean,
        val options: List<Option>,
        val default: JsonPrimitive?,
    ) : SchemaField {
        data class Option(val value: JsonPrimitive, val label: String)
    }
}

/** Why a value is not accepted. */
enum class FieldError { Required, TooShort, TooLong, Pattern, NotANumber, NotAnInteger, BelowMinimum, AboveMaximum, NotAnOption }

object SettingsSchema {
    private val json = Json { ignoreUnknownKeys = true }

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
        val enum = node["enum"] as? JsonArray
        val oneOf = node["oneOf"] as? JsonArray
        if (enum != null || oneOf != null) {
            val labels = (node["x-enum-labels"] as? JsonArray)?.map { (it as? JsonPrimitive)?.contentOrNull }
            val options = enum?.mapIndexedNotNull { i, v ->
                val p = v as? JsonPrimitive ?: return@mapIndexedNotNull null
                SchemaField.Choice.Option(p, labels?.getOrNull(i) ?: p.contentOrNull ?: p.toString())
            } ?: oneOf!!.mapNotNull { option ->
                val o = option as? JsonObject ?: return@mapNotNull null
                val value = o["const"] as? JsonPrimitive ?: return@mapNotNull null
                SchemaField.Choice.Option(value, o.string("title") ?: value.contentOrNull ?: value.toString())
            }
            return SchemaField.Choice(key, title, description, required, options, node["default"] as? JsonPrimitive)
        }
        return when (node.string("type")) {
            "string" -> SchemaField.Text(
                key, title, description, required,
                secret = node.string("format") == "password" || node.bool("writeOnly") == true || node.bool("x-secret") == true,
                url = node.string("format") == "uri" || node.string("format") == "url",
                placeholder = node.string("x-placeholder") ?: (node["examples"] as? JsonArray)?.firstOrNull()?.let { (it as? JsonPrimitive)?.contentOrNull },
                minLength = node.long("minLength")?.toInt(),
                maxLength = node.long("maxLength")?.toInt(),
                pattern = node.string("pattern"),
                default = node.string("default"),
            )
            "number", "integer" -> SchemaField.Number(
                key, title, description, required,
                integer = node.string("type") == "integer",
                minimum = node.double("minimum"),
                maximum = node.double("maximum"),
                default = node.double("default"),
            )
            "boolean" -> SchemaField.Toggle(key, title, description, required, node.bool("default") ?: false)
            else -> null
        }
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
            }
        }
    }

    /** Problems with [values]; empty when the form can be saved. */
    fun validate(fields: List<SchemaField>, values: Map<String, String>): Map<String, FieldError> {
        val errors = LinkedHashMap<String, FieldError>()
        for (f in fields) {
            val text = values[f.key].orEmpty()
            if (text.isEmpty()) {
                if (f.required && f !is SchemaField.Toggle) errors[f.key] = FieldError.Required
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
