import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.library)
}

android {
    namespace = "dev.spume.balance.shared"

    compileSdk {
        version = release(36)
    }

    defaultConfig {
        minSdk = 34
        consumerProguardFiles("consumer-rules.pro")
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }

    kotlin {
        compilerOptions {
            jvmTarget = JvmTarget.JVM_11
        }
    }

    // Everything under ../generated is produced by `just android/typegen`:
    // BoltFFI's Kotlin bindings and .so files, plus the app types from the
    // `codegen` binary.
    sourceSets {
        getByName("main") {
            kotlin.srcDirs("../generated")
            jniLibs.srcDirs("../generated/jniLibs")
        }
    }
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
}
