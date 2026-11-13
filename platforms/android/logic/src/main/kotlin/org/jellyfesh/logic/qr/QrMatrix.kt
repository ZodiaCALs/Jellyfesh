package org.jellyfesh.logic.qr

import io.nayuki.qrcodegen.QrCode

/**
 * A QR code as modules to draw, for the sign-in step that shows one (ADR-0004: the host renders
 * the QR natively). Encoded with Nayuki's QR Code generator (MIT), medium error correction.
 */
class QrMatrix private constructor(val size: Int, private val modules: BooleanArray) {
    /** True for a dark module at column [x], row [y]. */
    operator fun get(x: Int, y: Int): Boolean = x in 0 until size && y in 0 until size && modules[y * size + x]

    companion object {
        /** The QR code for [text], or null when it is too long for any QR version. */
        fun encode(text: String): QrMatrix? {
            val qr = runCatching { QrCode.encodeText(text, QrCode.Ecc.MEDIUM) }.getOrNull() ?: return null
            val size = qr.size
            val modules = BooleanArray(size * size) { qr.getModule(it % size, it / size) }
            return QrMatrix(size, modules)
        }

        /** The quiet zone around a code, in modules, that scanners need. */
        const val QUIET_ZONE = 4
    }
}
