package org.jellyfesh.logic.settings

import org.jellyfesh.logic.core.Command
import org.jellyfesh.logic.core.LicenceError
import org.jellyfesh.logic.core.ProSnapshot
import org.jellyfesh.logic.core.ProSource
import org.jellyfesh.logic.core.Store

/** The line Settings › Extensions › Jellyfesh Pro starts with (docs/ui.md). */
sealed interface ProLine {
    /** "Pro features are free during the public beta." */
    data object Preview : ProLine

    /** "Jellyfesh Pro · Purchased". */
    data object Purchased : ProLine

    /** "Jellyfesh Pro · Licence …ABCD". */
    data class Licence(val suffix: String) : ProLine

    /** "AI Lyrics and Customize are part of Jellyfesh Pro.", and the ways to get it. */
    data object Offer : ProLine
}

/**
 * Jellyfesh Pro as the app shows it (ADR-0021): the core decides whether Pro is active and where
 * it comes from (the `Pro` slice); this only reads the slice for the Pro group and the Pro sheet.
 */
object ProModel {
    fun line(pro: ProSnapshot): ProLine = when (val source = pro.source) {
        ProSource.Preview -> ProLine.Preview
        is ProSource.Purchased -> ProLine.Purchased
        is ProSource.Licence -> ProLine.Licence(source.idSuffix)
        ProSource.None -> ProLine.Offer
    }

    /**
     * Buy Jellyfesh Pro and Restore Purchases: while Pro is inactive, when [store] (the store this
     * build talks to, null for none) is one the core says this build sells Pro in.
     */
    fun sellsIn(pro: ProSnapshot, store: Store?): Boolean = !pro.active && store != null && store in pro.stores

    /** Enter Licence Key…: while Pro is inactive, in builds that take keys (not Google Play's). */
    fun takesLicenceKeys(pro: ProSnapshot): Boolean = !pro.active && pro.licenceKeys

    /** Remove Licence…: Pro comes from a licence key. */
    fun canRemoveLicence(pro: ProSnapshot): Boolean = pro.source is ProSource.Licence

    /** Whether a Pro action goes ahead; while Pro is inactive it opens the Pro sheet instead. */
    fun allows(pro: ProSnapshot): Boolean = pro.active
}

/** The core's answer to a licence key sent from Enter Licence Key. */
sealed interface LicenceAnswer {
    /** Nothing sent yet. */
    data object Idle : LicenceAnswer

    /** Sent; the core has not answered. */
    data object Waiting : LicenceAnswer

    /** Pro is on, from the key. */
    data object Accepted : LicenceAnswer

    /** The key was refused (the core's `licence_error`). */
    data class Refused(val error: LicenceError) : LicenceAnswer

    /** The command failed (its message, as the app words failures). */
    data class Failed(val message: String) : LicenceAnswer
}

/**
 * Enter Licence Key: the key goes to the core as typed (`ActivateLicence`; it ignores case, spaces
 * and dashes) and the answer arrives as a newer `Pro` slice: active from a licence, or with the
 * reason it was refused. [sentAt] is the slice's version when the key was sent, [commandId] the
 * command's id for a `CommandFailed`.
 */
data class LicenceEntry(val sentAt: Long? = null, val commandId: Long? = null, val failure: String? = null)

object LicenceEntries {
    /** The entry after sending [key] (blank: nothing to send), and the command to send. */
    fun send(pro: ProSnapshot, key: String): Pair<LicenceEntry, Command>? {
        val trimmed = key.trim()
        if (trimmed.isEmpty()) return null
        return LicenceEntry(sentAt = pro.version) to Command.ActivateLicence(trimmed)
    }

    /** The entry once the command has its id. */
    fun sent(entry: LicenceEntry, commandId: Long): LicenceEntry = entry.copy(commandId = commandId)

    /** A `CommandFailed` for [id]: the entry's failure when it is the key's command. */
    fun failed(entry: LicenceEntry, id: Long, message: String): LicenceEntry =
        if (entry.commandId != null && entry.commandId == id) entry.copy(failure = message) else entry

    fun answer(entry: LicenceEntry, pro: ProSnapshot): LicenceAnswer {
        entry.failure?.let { return LicenceAnswer.Failed(it) }
        val sentAt = entry.sentAt ?: return LicenceAnswer.Idle
        if (pro.version <= sentAt) return LicenceAnswer.Waiting
        if (pro.active && pro.source is ProSource.Licence) return LicenceAnswer.Accepted
        return pro.licenceError?.let { LicenceAnswer.Refused(it) } ?: LicenceAnswer.Waiting
    }
}
