package org.crownos.agent;

import android.util.Log;

import java.io.PrintStream;

final class Events {
    private static final String LOG_TAG = "CrownAgentSpike";

    private static volatile PrintStream sink = System.out;

    private Events() {
    }

    static void redirect(PrintStream stream) {
        sink = stream;
    }

    static long monotonicNanos() {
        return System.nanoTime();
    }

    static void emit(String event, String details) {
        String line = "t=" + monotonicNanos() + " " + event + (details.isEmpty() ? "" : " " + details);
        sink.println(line);
        Log.i(LOG_TAG, line);
    }

    static void error(Throwable failure) {
        Throwable root = failure;
        while (root.getCause() != null && root.getCause() != root) {
            root = root.getCause();
        }
        emit("error", "type=" + root.getClass().getSimpleName() + " message=\"" + failure.getMessage() + "\"");
    }
}
