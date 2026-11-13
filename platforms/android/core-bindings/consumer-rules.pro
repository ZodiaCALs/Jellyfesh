# JNA reads these classes by reflection and registers native methods by name.
-dontwarn java.awt.**
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.Structure { public *; }

# The UniFFI bindings (direct-mapped natives, callback vtables) and AndroidContext's JNI method.
-keep class org.jellyfesh.ffi.** { *; }
