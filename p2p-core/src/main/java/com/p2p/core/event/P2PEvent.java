package com.p2p.core.event;

import java.time.Instant;
import java.util.Objects;

/**
 * Base class for all events in the P2P event system.
 * Events are immutable and carry a timestamp and correlation ID.
 */
public abstract class P2PEvent {

    // --- Fields ---
    private final String eventType;
    private final Instant timestamp;
    private final String correlationId;

    // --- Constructors ---

    /**
     * Constructs a new P2PEvent.
     *
     * @param eventType     the event type identifier
     * @param correlationId correlation ID for tracing
     */
    protected P2PEvent(String eventType, String correlationId) {
        this.eventType = Objects.requireNonNull(eventType, "eventType required");
        this.timestamp = Instant.now();
        this.correlationId = Objects.requireNonNull(correlationId, "correlationId required");
    }

    // --- Getters ---

    /** Returns the event type identifier. */
    public String getEventType() { return eventType; }

    /** Returns the timestamp of when this event was created. */
    public Instant getTimestamp() { return timestamp; }

    /** Returns the correlation ID for tracing. */
    public String getCorrelationId() { return correlationId; }

    @Override
    public String toString() {
        return String.format("%s[time=%s, correlationId=%s]",
                eventType, timestamp, correlationId);
    }
}
