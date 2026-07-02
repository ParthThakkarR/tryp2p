plugins {
    `java-library`
}

description = "P2P Transfer - Transfer engine, chunking, compression, resume"

dependencies {
    api(project(":p2p-core"))
    implementation(project(":p2p-network"))
    implementation(project(":p2p-crypto"))
}
