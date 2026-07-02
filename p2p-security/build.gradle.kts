plugins {
    `java-library`
}

description = "P2P Security - Rate limiting, cert pinning, auth logging, firewall"

dependencies {
    api(project(":p2p-core"))
    implementation(project(":p2p-network"))
    implementation(project(":p2p-crypto"))
    implementation(rootProject.libs.jna)
    implementation(rootProject.libs.jna.platform)
}
