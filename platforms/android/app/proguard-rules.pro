# The core's bindings and JNA keep themselves (core-bindings/consumer-rules.pro).

# Media3 session service, found by the system through the manifest.
-keep class org.jellyfesh.android.playback.PlaybackService { *; }

# kotlinx.serialization's JSON tree is used without generated serializers; nothing to keep.
-dontwarn kotlinx.serialization.**
