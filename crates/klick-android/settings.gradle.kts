// kl!ck для Android: родное приложение (Kotlin/Compose). Движок — Rust (`src/`, libklick_android.so),
// ядро mihomo — Go (`../klick-ios/core`, libklickcore.so); обе библиотеки собирает scripts/android/build.sh
// в app/src/main/jniLibs до Gradle.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "klick"
include(":app")
