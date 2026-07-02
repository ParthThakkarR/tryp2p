package com.p2p.observability.errors;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.Objects;

/**
 * Sentry-backed {@link ErrorReporter}. Uses reflection to interact with the
 * Sentry SDK so that the dependency is optional at runtime.
 * Falls back to SLF4J logging when Sentry is unavailable or unconfigured.
 */
public final class SentryErrorReporter implements ErrorReporter {

    // --- Constants ---

    private static final Logger log = LoggerFactory.getLogger(SentryErrorReporter.class);
    private static final String SENTRY_DSN_PROPERTY = "p2p.sentry.dsn";
    private static final String SENTRY_CLASS_NAME = "io.sentry.Sentry";

    // --- Fields ---

    private final boolean sentryAvailable;
    private final boolean sentryConfigured;

    // --- Constructor ---

    /**
     * Creates a SentryErrorReporter. Detects the Sentry SDK on the classpath
     * and a DSN via the {@code p2p.sentry.dsn} system property.
     */
    public SentryErrorReporter() {
        this.sentryAvailable = isSentryOnClasspath();
        String dsn = System.getProperty(SENTRY_DSN_PROPERTY);
        this.sentryConfigured = sentryAvailable && dsn != null && !dsn.isBlank();
        if (sentryAvailable && sentryConfigured) {
            log.info("Sentry error reporter initialized with configured DSN");
        } else if (sentryAvailable) {
            log.info("Sentry found on classpath but not configured (set -D{}=<dsn>)", SENTRY_DSN_PROPERTY);
        } else {
            log.info("Sentry not available on classpath, falling back to logging");
        }
    }

    // --- Public API ---

    /**
     * Reports the error to Sentry if configured, otherwise falls back to SLF4J logging.
     *
     * @param context the full error context (must not be null)
     */
    @Override
    public void reportError(ErrorContext context) {
        Objects.requireNonNull(context, "error context must not be null");
        if (sentryConfigured) {
            reportToSentry(context);
        } else {
            reportToLog(context);
        }
    }

    // --- Private helpers ---

    private void reportToSentry(ErrorContext context) {
        try {
            Class<?> sentryClass = Class.forName(SENTRY_CLASS_NAME);
            Class<?> sentryEventClass = Class.forName("io.sentry.SentryEvent");
            Class<?> sentryLevelClass = Class.forName("io.sentry.SentryLevel");

            Object event = sentryEventClass.getDeclaredConstructor().newInstance();
            sentryEventClass.getMethod("setMessage", String.class).invoke(event, context.message());
            sentryEventClass.getMethod("setLevel", sentryLevelClass)
                    .invoke(event, sentryLevelClass.getField("ERROR").get(null));

            Object sentryException = sentryEventClass.getMethod("getThrowable").invoke(event);
            if (sentryException == null && context.stackTrace() != null) {
                sentryEventClass.getMethod("setTag", String.class, String.class)
                        .invoke(event, "stackTrace", context.stackTrace());
            }

            sentryEventClass.getMethod("setTag", String.class, String.class)
                    .invoke(event, "peerId", nullSafe(context.peerId()));
            sentryEventClass.getMethod("setTag", String.class, String.class)
                    .invoke(event, "sessionId", nullSafe(context.sessionId()));
            sentryEventClass.getMethod("setTag", String.class, String.class)
                    .invoke(event, "errorType", nullSafe(context.errorType()));
            sentryEventClass.getMethod("setTag", String.class, String.class)
                    .invoke(event, "fileInfo", nullSafe(context.fileInfo()));
            sentryEventClass.getMethod("setTag", String.class, String.class)
                    .invoke(event, "chunkInfo", nullSafe(context.chunkInfo()));

            sentryClass.getMethod("captureEvent", sentryEventClass).invoke(null, event);
        } catch (Exception e) {
            log.warn("Failed to report error to Sentry, falling back to log: {}", e.getMessage());
            reportToLog(context);
        }
    }

    private void reportToLog(ErrorContext context) {
        log.error("Error reported: type={}, message={}, peerId={}, sessionId={}, file={}, chunk={}",
                nullSafe(context.errorType()),
                nullSafe(context.message()),
                nullSafe(context.peerId()),
                nullSafe(context.sessionId()),
                nullSafe(context.fileInfo()),
                nullSafe(context.chunkInfo()));

        if (context.stackTrace() != null && !context.stackTrace().isBlank()) {
            log.error("Stack trace:\n{}", context.stackTrace());
        }
    }

    private static String nullSafe(String value) {
        return value != null ? value : "N/A";
    }

    private static String nullSafe(Object value) {
        return value != null ? value.toString() : "N/A";
    }

    private static boolean isSentryOnClasspath() {
        try {
            Class.forName(SENTRY_CLASS_NAME);
            return true;
        } catch (ClassNotFoundException e) {
            return false;
        }
    }
}
