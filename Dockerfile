# syntax=docker/dockerfile:1
FROM eclipse-temurin:21-jdk-alpine AS builder
WORKDIR /build
COPY gradle gradle
COPY gradlew build.gradle.kts settings.gradle.kts ./
COPY gradle/libs.versions.toml gradle/libs.versions.toml
COPY p2p-core p2p-core
COPY p2p-network p2p-network
COPY p2p-crypto p2p-crypto
COPY p2p-transfer p2p-transfer
COPY p2p-security p2p-security
COPY p2p-observability p2p-observability
COPY p2p-cli p2p-cli
COPY p2p-app p2p-app
RUN ./gradlew shadowJar --no-daemon

FROM eclipse-temurin:21-jre-alpine
RUN apk add --no-cache curl ca-certificates
WORKDIR /app
COPY --from=builder /build/p2p-app/build/libs/p2p-1.0.0-SNAPSHOT.jar /app/p2p.jar
RUN adduser -D p2p
USER p2p
EXPOSE 9877 9876 9090 9091
VOLUME ["/data", "/config"]
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:9091/health || exit 1
ENTRYPOINT ["java", "--enable-preview", "-jar", "/app/p2p.jar"]
