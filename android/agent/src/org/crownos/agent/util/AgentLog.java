package org.crownos.agent.util;

import android.util.Log;

import java.util.Set;
import java.util.concurrent.ConcurrentHashMap;

/** Logcat output under one tag. */
public final class AgentLog {
    private static final String TAG = "CrownAgent";
    private static final Set<String> REPORTED = ConcurrentHashMap.newKeySet();

    private AgentLog() {
    }

    public static void info(String message) {
        Log.i(TAG, message);
    }

    public static void warn(String message) {
        Log.w(TAG, message);
    }

    public static void error(String message, Throwable failure) {
        Log.e(TAG, message, failure);
    }

    /** Logs {@code message} the first time {@code key} fails, so periodic probes do not flood logcat. */
    public static void warnOnce(String key, String message) {
        if (REPORTED.add(key)) {
            Log.w(TAG, message);
        }
    }
}
