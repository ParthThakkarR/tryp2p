package com.p2p.observability.errors;

import java.util.Objects;

/**
 * No-op {@link ErrorReporter} that silently discards all error reports.
 * Useful for testing or when error reporting is disabled.
 */
public final class DevNullErrorReporter implements ErrorReporter {

    // --- Public API ---

    /**
     * Discards the error context without any action.
     *
     * @param context the error context (validated for null but otherwise ignored)
     */
    @Override
    public void reportError(ErrorContext context) {
        Objects.requireNonNull(context, "error context must not be null");
        // no-op
    }
}
