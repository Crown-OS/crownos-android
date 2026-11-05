package org.crownos.agent.sizing;

import java.util.HashMap;
import java.util.Map;

/**
 * Keeps at most one resize per task in flight and one pending behind it. A newer request replaces
 * the pending one, so a drag never queues more than the latest size.
 */
public final class ResizeCoalescer {
    private final Map<Integer, SizeRequest> inFlight = new HashMap<>();
    private final Map<Integer, SizeRequest> pending = new HashMap<>();

    /** Returns {@code request} when it may start now, or {@code null} when it waits behind one in flight. */
    public SizeRequest submit(SizeRequest request) {
        if (inFlight.containsKey(request.task)) {
            pending.put(request.task, request);
            return null;
        }
        inFlight.put(request.task, request);
        return request;
    }

    /** Whether {@code request} is the one in flight for its task. */
    public boolean isInFlight(SizeRequest request) {
        return inFlight.get(request.task) == request;
    }

    public boolean isBusy(int task) {
        return inFlight.containsKey(task);
    }

    /**
     * Ends {@code finished} and returns the pending request that starts now, or {@code null}. A stale
     * request, already ended or forgotten, changes nothing.
     */
    public SizeRequest complete(SizeRequest finished) {
        if (!isInFlight(finished)) {
            return null;
        }
        SizeRequest next = pending.remove(finished.task);
        if (next == null) {
            inFlight.remove(finished.task);
        } else {
            inFlight.put(finished.task, next);
        }
        return next;
    }

    public void forget(int task) {
        inFlight.remove(task);
        pending.remove(task);
    }
}
