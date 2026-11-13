// The transparency setting on Misty's glass (docs/design-controls.md, "Preferences → Appearance"):
// the shared glass-alpha port (platforms/shared/swift, replaying tests/contract/glass-alpha.json),
// compiled into the JellyfeshFFI module. This wrapper keeps that module inside this folder.

import JellyfeshFFI

enum GlassTransparency {
    /// The alpha of a glass grade whose token alpha is `tokenAlpha`, at `transparency` (0 solid …
    /// 1 clearest): the token's alpha at 0.5, opaque at 0, half of it at 1.
    static func alpha(_ tokenAlpha: Double, transparency: Double) -> Double {
        Double(glassAlpha(Float(tokenAlpha), transparency: Float(transparency)))
    }
}
