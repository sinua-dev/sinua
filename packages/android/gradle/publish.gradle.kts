// Maven publishing for the library modules (docs/publishing.md). Applied with
// `apply(from = rootProject.file("gradle/publish.gradle.kts"))` from each module.
//
// Nothing is published from here: this produces a local, complete artifact so
// `publishToMavenLocal` and a consumer build can be checked before the first release.
// The brand-dependent values (group, project URL, SCM) come from gradle.properties,
// as one place to edit once the name is decided -- they are not invented here.
plugins.apply("maven-publish")
plugins.apply("signing")

val publishGroup: String = (project.findProperty("sinua.group") as String?) ?: "dev.sinua"
// No default: gradle.properties carries it, written from the root VERSION file by
// scripts/release/set-version.mjs, so a build can't quietly publish another version.
val publishVersion: String = (project.findProperty("sinua.version") as String?)
    ?: error("sinua.version is missing from gradle.properties -- run node scripts/release/set-version.mjs")
val projectUrl: String = (project.findProperty("sinua.url") as String?) ?: "https://github.com/sinua-dev/sinua"
val scmConnection: String = (project.findProperty("sinua.scm") as String?) ?: "https://github.com/sinua-dev/sinua.git"

// The `release` variant that is published (with its sources + javadoc jars, which
// Maven Central requires) is declared in each module's own build file: the Android
// extension's types aren't on a script plugin's classpath.
afterEvaluate {
    extensions.configure(PublishingExtension::class.java) {
        publications.create("release", MavenPublication::class.java) {
            from(components["release"])
            groupId = publishGroup
            artifactId = (project.findProperty("sinua.artifactId") as String?)
                ?: project.name.removePrefix(":").ifEmpty { "core" }
            version = publishVersion
            pom {
                name.set(artifactId)
                description.set(
                    (project.findProperty("sinua.description") as String?)
                        ?: "Sinua: voice-reactive visuals from one shared engine",
                )
                url.set(projectUrl)
                licenses {
                    license {
                        name.set("The Apache License, Version 2.0")
                        url.set("https://www.apache.org/licenses/LICENSE-2.0.txt")
                    }
                }
                developers {
                    developer {
                        // The same authors string as NOTICE and the npm manifests.
                        id.set((project.findProperty("sinua.developerId") as String?) ?: "sinua")
                        name.set((project.findProperty("sinua.developerName") as String?) ?: "The Sinua Authors")
                        url.set(projectUrl)
                    }
                }
                scm {
                    connection.set("scm:git:$scmConnection")
                    developerConnection.set("scm:git:$scmConnection")
                    url.set(projectUrl)
                }
            }
        }
        // The release pipeline's staging repository (scripts/release/build.sh): one
        // Maven-layout directory for every module, zipped and uploaded to the Central
        // Portal by .github/workflows/release.yml. `-Psinua.stagingRepo=<dir>`.
        (project.findProperty("sinua.stagingRepo") as String?)?.let { dir ->
            repositories.maven {
                name = "staging"
                url = uri(dir)
            }
        }
    }
    // Central requires a detached signature (.asc) per file. Signed only when a key is
    // given (ORG_GRADLE_PROJECT_signingKey / signingPassword, an ASCII-armoured key in
    // memory, as in CI); a local or dry-run build stays unsigned and says so. Blank counts
    // as absent: GitHub Actions passes an unset secret as an empty string.
    val signingKey = (project.findProperty("signingKey") as String?)?.takeIf { it.isNotBlank() }
    if (signingKey != null) {
        extensions.configure(SigningExtension::class.java) {
            useInMemoryPgpKeys(signingKey, project.findProperty("signingPassword") as String?)
            sign(extensions.getByType(PublishingExtension::class.java).publications["release"])
        }
    } else if (gradle.startParameter.taskNames.any { it.contains("Staging") }) {
        logger.lifecycle("sinua: ${project.path} staged UNSIGNED (no signingKey) -- fine for a dry run, refused by Central")
    }
}
