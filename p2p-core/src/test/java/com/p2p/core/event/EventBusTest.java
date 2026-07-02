package com.p2p.core.event;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.BeforeEach;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

import static org.junit.jupiter.api.Assertions.*;

@DisplayName("EventBus Tests")
class EventBusTest {

    private EventBus eventBus;

    // Test event subclasses
    static class TestEventA extends P2PEvent {
        final String data;
        TestEventA(String data) { super("TEST_A", "corr-1"); this.data = data; }
    }

    static class TestEventB extends P2PEvent {
        final int value;
        TestEventB(int value) { super("TEST_B", "corr-2"); this.value = value; }
    }

    @BeforeEach
    void setUp() {
        eventBus = new EventBus();
    }

    @Test
    @DisplayName("publishSync delivers to subscribed listeners")
    void publishSyncDeliversToListeners() {
        AtomicReference<String> received = new AtomicReference<>();
        eventBus.subscribe(TestEventA.class, event -> received.set(event.data));

        eventBus.publishSync(new TestEventA("hello"));
        assertEquals("hello", received.get());
    }

    @Test
    @DisplayName("listeners only receive matching event types")
    void typeFiltering() {
        AtomicInteger countA = new AtomicInteger();
        AtomicInteger countB = new AtomicInteger();

        eventBus.subscribe(TestEventA.class, e -> countA.incrementAndGet());
        eventBus.subscribe(TestEventB.class, e -> countB.incrementAndGet());

        eventBus.publishSync(new TestEventA("x"));
        eventBus.publishSync(new TestEventA("y"));
        eventBus.publishSync(new TestEventB(42));

        assertEquals(2, countA.get());
        assertEquals(1, countB.get());
    }

    @Test
    @DisplayName("unsubscribe stops delivery")
    void unsubscribe() {
        AtomicInteger count = new AtomicInteger();
        EventListener<TestEventA> listener = e -> count.incrementAndGet();

        eventBus.subscribe(TestEventA.class, listener);
        eventBus.publishSync(new TestEventA("a"));
        assertEquals(1, count.get());

        eventBus.unsubscribe(TestEventA.class, listener);
        eventBus.publishSync(new TestEventA("b"));
        assertEquals(1, count.get()); // Should not increment
    }

    @Test
    @DisplayName("async publish delivers event")
    void asyncPublish() throws Exception {
        CountDownLatch latch = new CountDownLatch(1);
        AtomicReference<String> received = new AtomicReference<>();

        eventBus.subscribe(TestEventA.class, event -> {
            received.set(event.data);
            latch.countDown();
        });

        eventBus.publish(new TestEventA("async"));
        assertTrue(latch.await(2, TimeUnit.SECONDS));
        assertEquals("async", received.get());
    }

    @Test
    @DisplayName("listener exception does not affect other listeners")
    void listenerExceptionIsolation() {
        AtomicInteger successCount = new AtomicInteger();

        eventBus.subscribe(TestEventA.class, e -> {
            throw new RuntimeException("intentional failure");
        });
        eventBus.subscribe(TestEventA.class, e -> successCount.incrementAndGet());

        // publishSync should not throw
        assertDoesNotThrow(() -> eventBus.publishSync(new TestEventA("x")));
        assertEquals(1, successCount.get());
    }

    @Test
    @DisplayName("clear() removes all listeners")
    void clearRemovesAll() {
        AtomicInteger count = new AtomicInteger();
        eventBus.subscribe(TestEventA.class, e -> count.incrementAndGet());
        eventBus.clear();
        eventBus.publishSync(new TestEventA("x"));
        assertEquals(0, count.get());
    }

    @Test
    @DisplayName("event carries timestamp and correlationId")
    void eventProperties() {
        TestEventA event = new TestEventA("test");
        assertNotNull(event.getTimestamp());
        assertEquals("corr-1", event.getCorrelationId());
        assertEquals("TEST_A", event.getEventType());
    }
}
