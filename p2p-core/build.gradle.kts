plugins {
    `java-library`
}

description = "P2P Core - Domain models, interfaces, utilities"

dependencies {
    api(rootProject.libs.slf4j.api)
    implementation(rootProject.libs.bundles.logging)
}
