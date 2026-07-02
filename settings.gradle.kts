rootProject.name = "p2p-file-transfer"

pluginManagement {
    repositories {
        mavenCentral()
        gradlePluginPortal()
    }
}

@Suppress("UnstableApiUsage")
dependencyResolutionManagement {
    repositories {
        mavenCentral()
    }
}

include("p2p-core")
include("p2p-network")
include("p2p-crypto")
include("p2p-transfer")
include("p2p-security")
include("p2p-observability")
include("p2p-relay")
include("p2p-cli")
include("p2p-app")
