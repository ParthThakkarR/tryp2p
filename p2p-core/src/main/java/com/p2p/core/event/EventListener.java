package com.p2p.core.event;

/**
 * Listener interface for P2P events.
 *
 * @param <T> the specific event type this listener handles
 */
@FunctionalInterface
public interface EventListener<T extends P2PEvent> {

    /**
     * Called when an event of the matching type is published.
     *
     * @param event the event
     */
    void onEvent(T event);
}
