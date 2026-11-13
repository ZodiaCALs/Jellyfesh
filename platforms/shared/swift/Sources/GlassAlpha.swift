// Reference port of jf_palette::glass_alpha (core/jf-palette/src/theme.rs). Tests/GlassContractTests.swift
// replays tests/contract/glass-alpha.json; keep it in step with the Rust original. Swift 5 language
// mode; not compiled in this repository's CI yet (no Swift toolchain there): see docs/ffi.md.

/// The transparency setting that keeps the glass tokens' own alphas.
public let defaultTransparency: Float = 0.5

/// The alpha of a glass grade (Clear, Mist, Veil) whose token alpha is `alpha`, at the transparency
/// setting `transparency` (0 solid, 1 clearest).
///
/// At 0.5 the token's alpha applies unchanged; toward 0 it rises linearly to 1, so glass becomes
/// opaque; toward 1 it falls linearly to half the token's alpha. Inputs are clamped to 0...1; a
/// transparency that is not finite counts as 0.5 and an alpha that is not finite as 1
/// (docs/design-controls.md, "v0.0.4").
public func glassAlpha(_ alpha: Float, transparency: Float) -> Float {
    // In Double, so the ends and the default come out exact, as in the Rust original.
    let a = alpha.isFinite ? Double(min(max(alpha, 0), 1)) : 1
    let t = transparency.isFinite ? Double(min(max(transparency, 0), 1)) : 0.5
    let out = t <= 0.5 ? a + (1 - a) * (1 - t / 0.5) : a * (1 - (t - 0.5))
    return min(max(Float(out), 0), 1)
}
