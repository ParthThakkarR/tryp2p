package com.p2p.core.event;

import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.Executor;
import java.util.concurrent.Executors;

/**
 * Central event bus for publishing and subscribing to P2P events.
 * Thread-safe. Listeners are invoked asynchronously on a dedicated executor.
 */
public final class EventBus {

    private static final Logger log = LoggerFactory.getLogger(EventBus.class);

    // --- Fields ---

    private final Map<Class<? extends P2PEvent>, List<EventListener<? extends P2PEvent>>> listeners =
            new ConcurrentHashMap<>();

    private final Executor executor;

    // --- Constructors ---

    public EventBus() {
        this(Executors.newVirtualThreadPerTaskExecutor());
    }

    public EventBus(Executor executor) {
        this.executor = executor;
    }

    // --- Subscription ---

    /**
     * Subscribes a listener for a specific event type.
     *
     * @param eventType the event class to listen for
     * @param listener  the listener to invoke
     * @param <T>       the event type
     */
    @SuppressWarnings("unchecked")
    public <T extends P2PEvent> void subscribe(Class<T> eventType, EventListener<T> listener) {
        listeners.computeIfAbsent(eventType, k -> new CopyOnWriteArrayList<>())
                .add(listener);
        log.debug("Subscribed listener for event type: {}", eventType.getSimpleName());
    }

    /**
     * Unsubscribes a listener.
     *
     * @param eventType the event class
     * @param listener  the listener to remove
     * @param <T>       the event type
     */
    public <T extends P2PEvent> void unsubscribe(Class<T> eventType, EventListener<T> listener) {
        List<EventListener<? extends P2PEvent>> list = listeners.get(eventType);
        if (list != null) {
            list.remove(listener);
        }
    }

    // --- Publishing ---

    /**
     * Publishes an event to all registered listeners for its type.
     * Listeners are invoked asynchronously on the executor.
     *
     * @param event the event to publish
     * @param <T>   the event type
     */
    @SuppressWarnings("unchecked")
    public <T extends P2PEvent> void publish(T event) {
        List<EventListener<? extends P2PEvent>> list = listeners.get(event.getClass());
        if (list == null || list.isEmpty()) {
            log.trace("No listeners for event type: {}", event.getClass().getSimpleName());
            return;
        }

        for (EventListener<? extends P2PEvent> listener : list) {
            executor.execute(() -> {
                try {
                    ((EventListener<T>) listener).onEvent(event);
                } catch (Exception e) {
                    log.error("Error in event listener for {}: {}",
                            event.getClass().getSimpleName(), e.getMessage(), e);
                }
            });
        }
    }

    /**
     * Publishes an event synchronously (blocks until all listeners complete).
     * Use sparingly — primarily for testing.
     *
     * @param event the event to publish
     * @param <T>   the event type
     */
    @SuppressWarnings("unchecked")
    public <T extends P2PEvent> void publishSync(T event) {
        List<EventListener<? extends P2PEvent>> list = listeners.get(event.getClass());
        if (list == null) return;

        for (EventListener<? extends P2PEvent> listener : list) {
            try {
                ((EventListener<T>) listener).onEvent(event);
            } catch (Exception e) {
                log.error("Error in event listener for {}: {}",
                        event.getClass().getSimpleName(), e.getMessage(), e);
            }
        }
    }

    // --- Lifecycle ---

    /**
     * Removes all listeners.
     */
    public void clear() {
        listeners.clear();
    }
}
