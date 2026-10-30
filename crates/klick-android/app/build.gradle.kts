plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

// Версия — из Cargo.toml в корне (как у всего kl!ck), номер сборки — из CI (KLICK_BUILD).
val klickVersion: String = rootDir.resolve("../../Cargo.toml").readLines()
    .first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
val klickBuild: Int = System.getenv("KLICK_BUILD")?.toIntOrNull() ?: 1

android {
    namespace = "app.klick.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "app.klick.android"
        // Android 8: 98% живых телефонов, включая бюджетные китайские.
        minSdk = 26
        targetSdk = 35
        versionCode = klickBuild
        versionName = klickVersion
        // arm64 — почти все телефоны, armeabi-v7a — старые дешёвые, x86_64 — эмулятор в CI.
        ndk { abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64") }
    }

    // Подпись: ключ из CI (KLICK_KEYSTORE…), без него — отладочный ключ сборщика (поставить можно,
    // обновить поверх подписанной другим ключом — нет).
    val keystore = System.getenv("KLICK_KEYSTORE")?.let(::file)?.takeIf { it.exists() }
    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = keystore
                storePassword = System.getenv("KLICK_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("KLICK_KEY_ALIAS")
                keyPassword = System.getenv("KLICK_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }

    buildFeatures {
        compose = true
        buildConfig = true
    }

    // Наборы правил, каталог сервисов и база стран (scripts/android/build.sh кладёт их в assets) —
    // база стран уже сжата, а распаковывать при первом запуске быстрее несжатое.
    androidResources { noCompress += listOf("mmdb") }
    packaging { jniLibs { useLegacyPackaging = false } }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(composeBom)
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
}
