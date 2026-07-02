plugins {
    `java-library`
}

description = "P2P Relay - STUN, NAT traversal, global registry client, relay"

dependencies {
    api(project(":p2p-core"))
    api(project(":p2p-network"))
    api(project(":p2p-crypto"))
}

