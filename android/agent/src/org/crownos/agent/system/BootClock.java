package org.crownos.agent.system;

import android.os.Process;
import android.os.SystemClock;

import org.crownos.agent.platform.SystemProps;

/**
 * Android's boot time. The kernel clock counts from the host's boot, so the base is the moment
 * Android's init started ({@code ro.boottime.init}, CLOCK_BOOTTIME nanoseconds), or the agent's
 * own start when init did not record it.
 */
public final class BootClock {
    private static final long NANOS_PER_MILLI = 1_000_000L;

    private BootClock() {
    }

    public static int millisSinceInit() {
        long initNanos = SystemProps.getLong("ro.boottime.init", -1);
        long baseMillis = initNanos > 0 ? initNanos / NANOS_PER_MILLI : Process.getStartElapsedRealtime();
        long elapsed = SystemClock.elapsedRealtime() - baseMillis;
        return (int) Math.max(0, Math.min(elapsed, Integer.MAX_VALUE));
    }
}
