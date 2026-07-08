plugins {
    `java-library`
}

description = "P2P CLI - Command-line interface"

dependencies {
    implementation(project(":p2p-core"))
    implementation(project(":p2p-network"))
    implementation(project(":p2p-crypto"))
    implementation(project(":p2p-transfer"))
    implementation(project(":p2p-security"))
    implementation(project(":p2p-observability"))
    api(rootProject.libs.picocli)
    annotationProcessor(rootProject.libs.picocli.codegen)
    implementation(rootProject.libs.picocli.codegen)
}
