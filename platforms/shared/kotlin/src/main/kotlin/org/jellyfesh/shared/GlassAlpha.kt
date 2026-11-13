// Reference port of jf_palette::glass_alpha (core/jf-palette/src/theme.rs). Tested against
// tests/contract/glass-alpha.json; keep it in step with the Rust original.

package org.jellyfesh.shared

/** The transparency setting that keeps the glass tokens' own alphas. */
const val DEFAULT_TRANSPARENCY: Float = 0.5f

/**
 * The alpha of a glass grade (Clear, Mist, Veil) whose token alpha is [alpha], at the
 * transparency setting [transparency] (0 solid, 1 clearest).
 *
 * At 0.5 the token's alpha applies unchanged; toward 0 it rises linearly to 1, so glass becomes
 * opaque; toward 1 it falls linearly to half the token's alpha. Inputs are clamped to 0..1; a
 * transparency that is not finite counts as 0.5 and an alpha that is not finite as 1
 * (docs/design-controls.md, "v0.0.4").
 */
fun glassAlpha(alpha: Float, transparency: Float): Float {
    // In Double, so the ends and the default come out exact, as in the Rust original.
    val a = if (alpha.isFinite()) alpha.coerceIn(0f, 1f).toDouble() else 1.0
    val t = if (transparency.isFinite()) transparency.coerceIn(0f, 1f).toDouble() else 0.5
    val out = if (t <= 0.5) a + (1.0 - a) * (1.0 - t / 0.5) else a * (1.0 - (t - 0.5))
    return out.toFloat().coerceIn(0f, 1f)
}
