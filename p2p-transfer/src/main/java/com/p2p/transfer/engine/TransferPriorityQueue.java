package com.p2p.transfer.engine;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Deque;
import java.util.Iterator;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.locks.ReentrantLock;

/**
 * A thread-safe priority queue for transfer tasks with two tiers: normal and
 * priority. Items in the priority tier are always dequeued first. All public
 * operations are guarded by a {@link ReentrantLock}.
 *
 * @param <T> the type of items in the queue
 */
public final class TransferPriorityQueue<T> {

    // --- Fields ---

    private final Deque<T> normalQueue = new ArrayDeque<>();
    private final Deque<T> priorityQueue = new ArrayDeque<>();
    private final ReentrantLock lock = new ReentrantLock();

    // --- Public API ---

    /**
     * Adds an item to the normal-priority queue.
     *
     * @param item the item to add
     * @throws NullPointerException if item is null
     */
    public void offer(T item) {
        Objects.requireNonNull(item, "item must not be null");
        lock.lock();
        try {
            normalQueue.addLast(item);
        } finally {
            lock.unlock();
        }
    }

    /**
     * Adds an item to the priority queue (processed before normal items).
     *
     * @param item the item to add
     * @throws NullPointerException if item is null
     */
    public void offerPriority(T item) {
        Objects.requireNonNull(item, "item must not be null");
        lock.lock();
        try {
            priorityQueue.addFirst(item);
        } finally {
            lock.unlock();
        }
    }

    /**
     * Retrieves and removes the head of this queue. Priority items are
     * returned before normal items.
     *
     * @return the head item, or null if the queue is empty
     */
    public T poll() {
        lock.lock();
        try {
            if (!priorityQueue.isEmpty()) {
                return priorityQueue.pollFirst();
            }
            return normalQueue.pollFirst();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Retrieves, but does not remove, the head of this queue.
     *
     * @return the head item, or null if the queue is empty
     */
    public T peek() {
        lock.lock();
        try {
            if (!priorityQueue.isEmpty()) {
                return priorityQueue.peekFirst();
            }
            return normalQueue.peekFirst();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns true if this queue contains no items.
     *
     * @return true if empty
     */
    public boolean isEmpty() {
        lock.lock();
        try {
            return priorityQueue.isEmpty() && normalQueue.isEmpty();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns the total number of items in this queue (priority + normal).
     *
     * @return item count
     */
    public int size() {
        lock.lock();
        try {
            return priorityQueue.size() + normalQueue.size();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns the number of items in the priority tier.
     *
     * @return priority item count
     */
    public int prioritySize() {
        lock.lock();
        try {
            return priorityQueue.size();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns the number of items in the normal tier.
     *
     * @return normal item count
     */
    public int normalSize() {
        lock.lock();
        try {
            return normalQueue.size();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns a snapshot list of all items, priority items first, then normal
     * items, in iteration order.
     *
     * @return a new list containing all items
     */
    public List<T> toList() {
        lock.lock();
        try {
            List<T> result = new ArrayList<>();
            result.addAll(priorityQueue);
            result.addAll(normalQueue);
            return result;
        } finally {
            lock.unlock();
        }
    }

    /**
     * Removes all items from this queue.
     */
    public void clear() {
        lock.lock();
        try {
            priorityQueue.clear();
            normalQueue.clear();
        } finally {
            lock.unlock();
        }
    }

    /**
     * Returns an iterator over a snapshot of the queue items.
     *
     * @return an iterator
     */
    public Iterator<T> iterator() {
        return toList().iterator();
    }

    // --- Object ---

    @Override
    public String toString() {
        lock.lock();
        try {
            return String.format("TransferPriorityQueue[total=%d, priority=%d, normal=%d]",
                    priorityQueue.size() + normalQueue.size(),
                    priorityQueue.size(),
                    normalQueue.size());
        } finally {
            lock.unlock();
        }
    }
}
