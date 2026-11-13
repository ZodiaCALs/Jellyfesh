package org.jellyfesh.shared

import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** The shared contract files in tests/contract/. */
object Contract {
    val repoRoot: File = File(System.getProperty("jellyfesh.repoRoot") ?: "../../..")

    fun read(name: String): JsonObject =
        Json.parseToJsonElement(repoRoot.resolve("tests/contract/$name").readText()).jsonObject
}

fun JsonObject.field(name: String): JsonElement = get(name) ?: error("no field $name in $this")

fun JsonElement.isNull(): Boolean = this is JsonNull

fun JsonElement.text(): String = jsonPrimitive.content

fun JsonElement.long(): Long = jsonPrimitive.content.toLong()

fun JsonElement.ulong(): ULong = jsonPrimitive.content.toULong()

fun JsonElement.longOrNull(): Long? = if (isNull()) null else long()

fun JsonElement.intOrNull(): Int? = if (isNull()) null else jsonPrimitive.content.toInt()

fun JsonElement.uints(): List<UInt> = jsonArray.map { it.jsonPrimitive.content.toUInt() }

fun JsonElement.texts(): List<String> = jsonArray.map { it.text() }
