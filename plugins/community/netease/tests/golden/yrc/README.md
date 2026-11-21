# YRC golden cases

Invented test data for the conversion of NetEase's word-timed lyrics (YRC)
to enhanced LRC (`src/yrc.rs`, `src/yrc_golden.rs`). None of it is from a
real song: the words, times and credits were made up for these tests.

* `<case>.yrc`: the input, in the format NetEase's lyric answers use for
  `yrc.lyric` (a first `#` line says what the case is about).
* `<case>.trans.lrc`: a translation, as NetEase sends it in LRC (optional).
* `<case>.elrc`: the expected enhanced LRC. Regenerate with
  `UPDATE_GOLDEN=1 cargo test -p jf-community-netease yrc_golden` and review
  the diff.
