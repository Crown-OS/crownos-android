package org.crownos.agent;

import android.graphics.Rect;
import android.view.SurfaceControl;
import android.window.WindowContainerTransactionCallback;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

final class Resize {
    private static final String TRANSACTION_CLASS = "android.window.WindowContainerTransaction";
    private static final int RESIZE_MODE_PRESERVE_WINDOW = 1;
    private static final long SYNC_WAIT_LIMIT_MS = 6000;
    private static final double NANOS_PER_MILLI = 1_000_000.0;

    private Resize() {
    }

    static void withTransaction(int taskId, Rect bounds, boolean synchronous) {
        Object transaction = boundsTransaction(TaskRecord.of(TaskQueries.require(taskId)).token, bounds);
        if (synchronous) {
            applySynchronously(taskId, bounds, transaction);
            return;
        }
        long commandNanos = Events.monotonicNanos();
        Hidden.call(Services.windowOrganizerController(), "applyTransaction",
                new Class<?>[] {Hidden.type(TRANSACTION_CLASS)}, transaction);
        Events.emit("resize.wct", "task=" + taskId + " bounds=" + Rects.format(bounds) + " t_cmd=" + commandNanos
                + " call_ms=" + millisSince(commandNanos));
    }

    static void legacy(int taskId, Rect bounds) {
        long commandNanos = Events.monotonicNanos();
        boolean resized = (Boolean) Hidden.call(Services.activityTaskManager(), "resizeTask",
                new Class<?>[] {int.class, Rect.class, int.class}, taskId, bounds, RESIZE_MODE_PRESERVE_WINDOW);
        Events.emit("resize.legacy", "task=" + taskId + " bounds=" + Rects.format(bounds) + " result=" + resized
                + " t_cmd=" + commandNanos + " call_ms=" + millisSince(commandNanos));
    }

    private static Object boundsTransaction(Object token, Rect bounds) {
        Object transaction = Hidden.construct(TRANSACTION_CLASS);
        Hidden.call(transaction, "setBounds",
                new Class<?>[] {Hidden.type("android.window.WindowContainerToken"), Rect.class}, token, bounds);
        return transaction;
    }

    private static void applySynchronously(int taskId, Rect bounds, Object transaction) {
        ReadyCallback callback = new ReadyCallback();
        Object organizer = Hidden.construct("android.window.WindowOrganizer");
        long commandNanos = Events.monotonicNanos();
        int syncId = (Integer) Hidden.call(organizer, "applySyncTransaction",
                new Class<?>[] {Hidden.type(TRANSACTION_CLASS), WindowContainerTransactionCallback.class},
                transaction, callback);
        boolean ready = callback.await();
        Events.emit("resize.wct.sync", "task=" + taskId + " bounds=" + Rects.format(bounds) + " sync_id=" + syncId
                + " ready=" + ready + " t_cmd=" + commandNanos + " t_ready=" + callback.readyNanos
                + " latency_ms=" + (ready ? String.valueOf((callback.readyNanos - commandNanos) / NANOS_PER_MILLI)
                        : "timeout"));
    }

    private static String millisSince(long startNanos) {
        return String.valueOf((Events.monotonicNanos() - startNanos) / NANOS_PER_MILLI);
    }

    private static final class ReadyCallback extends WindowContainerTransactionCallback {
        private final CountDownLatch readySignal = new CountDownLatch(1);
        private volatile long readyNanos;

        @Override
        public void onTransactionReady(int syncId, SurfaceControl.Transaction transaction) {
            readyNanos = Events.monotonicNanos();
            transaction.apply();
            readySignal.countDown();
        }

        boolean await() {
            try {
                return readySignal.await(SYNC_WAIT_LIMIT_MS, TimeUnit.MILLISECONDS);
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                return false;
            }
        }
    }
}
