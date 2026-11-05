package org.crownos.agent.parking;

import java.util.Collections;
import java.util.HashMap;
import java.util.HashSet;
import java.util.Iterator;
import java.util.Map;
import java.util.Set;
import java.util.function.LongSupplier;

/**
 * Decides when Back closed a window while the app keeps running. Android 12 and later move a root
 * task to the back on Back instead of finishing it; on the desktop that is closing the app's last
 * window. A task that leaves the front within {@link #BACK_WINDOW_MILLIS} of a Back is parked: it
 * has no window until it comes to the front again.
 */
public final class BackParking {
    public static final long BACK_WINDOW_MILLIS = 1_000;

    private final LongSupplier clockMillis;
    private final Map<Integer, Long> backPressedAt = new HashMap<>();
    private final Set<Integer> parked = new HashSet<>();

    public BackParking(LongSupplier clockMillis) {
        this.clockMillis = clockMillis;
    }

    /** The host sent Back for {@code task}. */
    public void backPressed(int task) {
        long now = clockMillis.getAsLong();
        dropExpired(now);
        backPressedAt.put(task, now);
    }

    /** The host focused {@code task}, which explains any other task losing focus now. */
    public void hostFocused(int task) {
        backPressedAt.keySet().retainAll(Collections.singleton(task));
    }

    /**
     * {@code task} left the front: it moved behind, lost focus while still running, or became
     * invisible. Returns {@code true} when this follows a Back closely enough to park it.
     */
    public boolean leftFront(int task) {
        Long pressed = backPressedAt.remove(task);
        if (pressed == null || clockMillis.getAsLong() - pressed > BACK_WINDOW_MILLIS) {
            return false;
        }
        parked.add(task);
        return true;
    }

    /** {@code task} came back to the front. Returns {@code true} when it was parked and is a window again. */
    public boolean cameToFront(int task) {
        backPressedAt.remove(task);
        return parked.remove(task);
    }

    public boolean isParked(int task) {
        return parked.contains(task);
    }

    /** {@code task} is gone from Android. */
    public void forget(int task) {
        backPressedAt.remove(task);
        parked.remove(task);
    }

    private void dropExpired(long now) {
        for (Iterator<Long> pressed = backPressedAt.values().iterator(); pressed.hasNext();) {
            if (now - pressed.next() > BACK_WINDOW_MILLIS) {
                pressed.remove();
            }
        }
    }
}
