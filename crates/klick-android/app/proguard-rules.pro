# Rust (JNI) зовёт эти классы и методы по именам: не переименовывать и не выкидывать.
-keep class app.klick.android.Engine { *; }
-keep class app.klick.android.Bridge { *; }
-keep class app.klick.android.TunnelCore { *; }
