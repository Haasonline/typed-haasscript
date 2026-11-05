import org.jetbrains.intellij.platform.gradle.IntelliJPlatformType
import org.jetbrains.intellij.platform.gradle.TestFrameworkType

plugins {
    id("org.jetbrains.kotlin.jvm") version "2.4.21"
    id("org.jetbrains.intellij.platform") version "2.20.0"
}

group = "com.haasonline.typedhaasscript"
version = providers.gradleProperty("pluginVersion").get()

kotlin {
    jvmToolchain(21)
}

repositories {
    mavenCentral()
    intellijPlatform {
        defaultRepositories()
    }
}

dependencies {
    intellijPlatform {
        pycharm(providers.gradleProperty("platformVersion"))
        bundledPlugin("org.jetbrains.plugins.textmate")
        testFramework(TestFrameworkType.Platform)
    }
    testImplementation("junit:junit:4.13.2")
    testImplementation("org.opentest4j:opentest4j:1.3.0")
}

intellijPlatform {
    pluginConfiguration {
        version = providers.gradleProperty("pluginVersion")
        ideaVersion {
            sinceBuild = providers.gradleProperty("sinceBuild")
            untilBuild = provider { null }
        }
    }
    publishing {
        token = providers.environmentVariable("JETBRAINS_MARKETPLACE_TOKEN")
        channels = providers.gradleProperty("publishChannel").map { listOf(it) }
    }
    // The release built against and the newest, as PyCharm: a free-tier
    // IDE, so the plugin is checked where most owners run it. While both are
    // the same release, the verifier checks it once.
    pluginVerification {
        ides {
            create(IntelliJPlatformType.PyCharm, providers.gradleProperty("platformVersion"))
            create(IntelliJPlatformType.PyCharm, providers.gradleProperty("newestVerifiedVersion"))
        }
    }
}

// The TextMate bundle is the VSCode extension's own grammar and language
// settings, so both editors highlight alike. It goes into the plugin's
// directory, where the TextMate plugin reads it from disk.
val vscodeExtension = layout.projectDirectory.dir("../vscode")
val textMateBundle = copySpec {
    from(vscodeExtension) {
        include("package.json", "language-configuration.json", "syntaxes/**")
    }
    into("textmate")
}

tasks {
    prepareSandbox {
        into(pluginName) { with(textMateBundle) }
    }
    prepareTestSandbox {
        into(pluginName) { with(textMateBundle) }
    }
}
