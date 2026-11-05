package org.crownos.agent.system;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/**
 * Readies Android for the container stop that follows a Shutdown, the way ShutdownThread does:
 * {@code IActivityManager.shutdown} pauses and stops every activity, so apps save their state, and
 * persists recents, usage and battery stats.
 */
public final class ShutdownPreparation {
    private static final int ACTIVITY_STOP_WAIT_MILLIS = 3_000;

    private ShutdownPreparation() {
    }

    public static void run() {
        try {
            Object timedOut = Hidden.call(Services.activityManager(), "shutdown", Hidden.types(int.class),
                    ACTIVITY_STOP_WAIT_MILLIS);
            AgentLog.info("activities stopped for shutdown" + (Boolean.TRUE.equals(timedOut) ? " (timed out)" : ""));
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot prepare activities for shutdown: " + e.getMessage());
        }
    }
}
