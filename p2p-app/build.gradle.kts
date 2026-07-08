plugins {
    application
    alias(libs.plugins.shadow)
}

description = "P2P App - Application bootstrap and wiring"

application {
    mainClass.set("com.p2p.app.P2PApplication")
}

dependencies {
    implementation(project(":p2p-core"))
    implementation(project(":p2p-network"))
    implementation(project(":p2p-crypto"))
    implementation(project(":p2p-transfer"))
    implementation(project(":p2p-security"))
    implementation(project(":p2p-observability"))
    implementation(project(":p2p-cli"))
    implementation(rootProject.libs.bundles.logging)
}

tasks.shadowJar {
    archiveBaseName.set("p2p")
    archiveClassifier.set("")
    mergeServiceFiles()
    manifest {
        attributes(
            "Main-Class" to "com.p2p.app.P2PApplication",
            "Implementation-Title" to "P2P File Transfer",
            "Implementation-Version" to project.version
        )
    }
}

val shadowJarFile = tasks.shadowJar.flatMap { it.archiveFile }
val jpackageOutput = layout.buildDirectory.dir("dist-jpackage")
val appImageDir = jpackageOutput.map { it.dir("P2PTransfer") }
val iconFile = layout.buildDirectory.file("icon.ico")

// --- Generate a simple application icon (.ico) using Java2D ---
val iconProvider = tasks.register<JavaExec>("generateIcon") {
    description = "Generate application .ico file"
    group = "build"
    classpath = sourceSets.main.get().runtimeClasspath
    mainClass.set("com.p2p.app.IconGenerator")
    args(iconFile.get().asFile.absolutePath)
    outputs.file(iconFile)
    dependsOn(tasks.classes)
}

abstract class JpackageTask : DefaultTask() {
    @get:InputFile
    abstract val shadowJar: RegularFileProperty

    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty

    @get:Input
    abstract val packageType: Property<String>

    @get:Optional
    @get:InputFile
    abstract val icon: RegularFileProperty

    @TaskAction
    fun exec() {
        val javaHome = System.getProperty("java.home")
        val args = mutableListOf(
            "$javaHome/bin/jpackage",
            "--type", packageType.get(),
            "--input", shadowJar.get().asFile.parentFile.absolutePath,
            "--main-jar", shadowJar.get().asFile.name,
            "--main-class", "com.p2p.app.P2PApplication",
            "--name", "P2PTransfer",
            "--dest", outputDir.get().asFile.absolutePath,
            "--vendor", "P2P Team",
            "--app-version", project.version.toString().removeSuffix("-SNAPSHOT"),
            "--win-console",
            "--java-options", "--enable-native-access=ALL-UNNAMED"
        )
        if (icon.isPresent) {
            args.add("--icon")
            args.add(icon.get().asFile.absolutePath)
        }
        if (packageType.get() == "msi" || packageType.get() == "exe") {
            args.add("--win-menu")
            args.add("--win-shortcut")
            args.add("--temp")
            args.add(outputDir.get().asFile.resolve("jpackage-work").absolutePath)
        }
        val targetDir = outputDir.get().asFile
        val newArgs = args.toMutableList()
        val destIndex = newArgs.indexOf("--dest") + 1
        newArgs[destIndex] = targetDir.absolutePath
        // Remove stale output if it exists
        if (targetDir.exists()) {
            targetDir.deleteRecursively()
        }
        val pb = ProcessBuilder(newArgs)
        // Point WiX v5 to the Util extension (needed when running from a subdirectory)
        if (packageType.get() == "msi" || packageType.get() == "exe") {
            val dotnetTools = File(System.getProperty("user.home"), ".dotnet\\tools")
            if (dotnetTools.exists()) {
                val pathEnv = pb.environment().getOrDefault("PATH", "")
                pb.environment().put("PATH", "$pathEnv;${dotnetTools.absolutePath}")
            }
            val wixDir = project.rootProject.projectDir.toPath().resolve(".wix").resolve("extensions")
            if (wixDir.toFile().exists()) {
                pb.environment().put("WIX_EXTENSIONS_DIR", wixDir.toAbsolutePath().toString())
            }
        }
        pb.redirectErrorStream(true)
        val process = pb.start()
        val output = process.inputStream.bufferedReader().readText()
        val exitCode = process.waitFor()
        if (exitCode != 0) {
            throw GradleException("jpackage failed with exit code $exitCode:\n$output")
        }
        logger.lifecycle(output)
    }
}

tasks.register<JpackageTask>("packageWinApp") {
    dependsOn("shadowJar", iconProvider)
    description = "Create portable Windows app image using jpackage (no extra tools needed)"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(jpackageOutput)
    packageType.set("app-image")
    icon.set(iconFile)
}

tasks.register<Zip>("packageWinZip") {
    dependsOn("packageWinApp")
    description = "Create portable ZIP distribution of the Windows app image"
    group = "distribution"
    from(appImageDir)
    archiveBaseName.set("P2PTransfer")
    archiveVersion.set(project.version.toString())
    archiveExtension.set("zip")
    destinationDirectory.set(layout.buildDirectory.dir("distributions"))
}

tasks.register<JpackageTask>("packageWinExe") {
    dependsOn("shadowJar", iconProvider)
    description = "Create Windows EXE installer (requires WiX Toolset installed)"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-exe"))
    packageType.set("exe")
    icon.set(iconFile)
}

tasks.register<JpackageTask>("packageWinMsi") {
    dependsOn("shadowJar", iconProvider)
    description = "Create Windows MSI installer (requires WiX Toolset installed)"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-msi"))
    packageType.set("msi")
    icon.set(iconFile)
}

// --- Portable single-file EXE ---
// Builds the app-image, then wraps it into a self-extracting portable EXE
// using 7-Zip SFX. The user downloads ONE .exe and runs it directly.
// No install needed, no separate runtime folder.
// Requires 7-Zip (7z.exe) in PATH.

abstract class PortableExeTask : DefaultTask() {
    @get:InputDirectory
    abstract val inputAppImage: DirectoryProperty

    @get:OutputFile
    abstract val outputExe: RegularFileProperty

    @get:Optional
    @get:InputFile
    abstract val icon: RegularFileProperty

    @TaskAction
    fun exec() {
        val appDir = inputAppImage.get().asFile
        if (!appDir.exists()) throw GradleException("App image not found: $appDir")

        val tmpDir = temporaryDir
        val sfxConfig = File(tmpDir, "~config.txt")
        sfxConfig.writeText("""
;!@Install@!UTF-8!
Title="P2P File Transfer"
RunProgram="${appDir.name}\\P2PTransfer.exe"
Directory="%%T\\P2PTransfer"
;!@InstallEnd@!
        """.trimIndent())

        val dest = outputExe.get().asFile
        dest.parentFile.mkdirs()
        val sfx7zPath = "C:\\Program Files\\7-Zip\\7z.sfx"

        // Create a custom SFX module with our icon (before the 7z archive is appended)
        val sfxModule = if (icon.isPresent) {
            val sfxOrig = File(sfx7zPath)
            if (!sfxOrig.exists()) throw GradleException("7z.sfx not found at $sfx7zPath")
            val customSfx = File(tmpDir, "p2p.sfx")
            sfxOrig.copyTo(customSfx, overwrite = true)
            val rceditPath = try {
                ProcessBuilder("where", "rcedit").redirectErrorStream(true).start().let { p ->
                    p.waitFor(); p.inputStream.bufferedReader().readText().lines().firstOrNull { it.isNotBlank() }
                }
            } catch (e: Exception) { null }
            if (rceditPath != null) {
                logger.lifecycle("  Applying icon to SFX module...")
                val pbRc = ProcessBuilder(rceditPath, customSfx.absolutePath, "--set-icon", icon.get().asFile.absolutePath)
                pbRc.redirectErrorStream(true)
                val procRc = pbRc.start()
                val outRc = procRc.inputStream.bufferedReader().readText()
                val codeRc = procRc.waitFor()
                if (codeRc != 0) logger.warn("  rcedit warning: $outRc")
                else logger.lifecycle("  Icon applied to SFX module.")
            }
            customSfx
        } else null

        // Build the SFX EXE: custom SFX + config + 7z archive
        // Step 1: Create plain 7z archive (no SFX, no config inside)
        val archive7z = File(tmpDir, "archive.7z")
        var pb = ProcessBuilder(
            "7z", "a", "-mx3", "-mmt2",
            archive7z.absolutePath,
            appDir.absolutePath + "\\*"
        )
        pb.directory(appDir.parentFile)
        pb.redirectErrorStream(true)
        var proc = pb.start()
        var out = proc.inputStream.bufferedReader().readText()
        var code = proc.waitFor()
        if (code != 0) throw GradleException("7-Zip archive failed:${System.lineSeparator()}$out")

        // Step 2: Concatenate SFX module + config + 7z archive
        // The config MUST be between the SFX module and the 7z data so the
        // SFX module can read it BEFORE extraction (sets Title, RunProgram, etc.)
        val sfxToUse = sfxModule ?: File(sfx7zPath)
        dest.outputStream().use { outStream ->
            outStream.write(sfxToUse.readBytes())
            outStream.write(sfxConfig.readText().toByteArray(Charsets.UTF_8))
            outStream.write("\r\n".toByteArray())
            outStream.write(archive7z.readBytes())
        }

        logger.lifecycle("Portable EXE created: ${dest.absolutePath} (${dest.length() / 1024 / 1024} MB)")
    }
}

tasks.register<PortableExeTask>("packageWinPortable") {
    dependsOn("packageWinApp", iconProvider)
    description = "Create truly portable single-file EXE (requires 7-Zip)"
    group = "distribution"
    inputAppImage.set(appImageDir)
    outputExe.set(layout.buildDirectory.file("dist-portable/P2PTransfer-Portable-${project.version}.exe"))
    icon.set(iconFile)
}

// --- Cross-platform packaging tasks ---
// These rely on jpackage, which is bundled with JDK 21+.
// Each task must be run ON ITS TARGET PLATFORM.

tasks.register<JpackageTask>("packageMacApp") {
    dependsOn("shadowJar")
    description = "Create macOS app bundle using jpackage"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-mac-app"))
    packageType.set("app-image")
}

tasks.register<JpackageTask>("packageMacDmg") {
    dependsOn("shadowJar")
    description = "Create macOS DMG installer"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-mac-dmg"))
    packageType.set("dmg")
}

tasks.register<JpackageTask>("packageLinuxApp") {
    dependsOn("shadowJar")
    description = "Create Linux app image using jpackage"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-linux-app"))
    packageType.set("app-image")
}

tasks.register<JpackageTask>("packageLinuxDeb") {
    dependsOn("shadowJar")
    description = "Create Linux DEB package (Debian/Ubuntu)"
    group = "distribution"
    shadowJar.set(shadowJarFile)
    outputDir.set(layout.buildDirectory.dir("dist-linux-deb"))
    packageType.set("deb")
}
