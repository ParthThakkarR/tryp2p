plugins {
    `java-library`
}

description = "P2P Observability - Metrics, structured logging, health, audit"

dependencies {
    api(project(":p2p-core"))
    implementation(rootProject.libs.micrometer.core)
    implementation(rootProject.libs.micrometer.prometheus)
}
