package org.crownos.agent.task;

import android.graphics.Rect;
import android.view.SurfaceControl;

import org.crownos.agent.atlas.Atlas;
import org.crownos.agent.atlas.Slot;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.sizing.ResizeCoalescer;
import org.crownos.agent.sizing.SizeRequest;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;

/**
 * Moves tasks into atlas slots. Each size goes through {@code applySyncTransaction}, so the slot is
 * reported only after the task redrew at that size, with its leash moved in the same transaction.
 * Falls back to a plain transaction, then to {@code resizeTask}.
 */
final class TaskResizer {
    /** Hears that a task now shows its slot. */
    interface Listener {
        void onSlotApplied(TaskState task);
    }

    /** Longer than the framework's own 5 s BLAST sync timeout, which normally answers first. */
    private static final long SYNC_TIMEOUT_MILLIS = 6_000;
    private static final int RESIZE_MODE_PRESERVE_WINDOW = 1;

    private final Loop loop;
    private final Atlas atlas;
    private final Listener listener;
    private final ResizeCoalescer coalescer = new ResizeCoalescer();

    TaskResizer(Loop loop, Atlas atlas, Listener listener) {
        this.loop = loop;
        this.atlas = atlas;
        this.listener = listener;
    }

    void request(TaskState task, int width, int height, int serial) {
        SizeRequest started = coalescer.submit(new SizeRequest(task.id, width, height, serial));
        if (started != null) {
            apply(task, started);
        }
    }

    boolean isBusy(int taskId) {
        return coalescer.isBusy(taskId);
    }

    void forget(int taskId) {
        coalescer.forget(taskId);
    }

    private void apply(TaskState task, SizeRequest request) {
        Slot slot = atlas.place(task.id, request.width, request.height);
        task.slot = slot;
        TaskSnapshot snapshot = task.snapshot;
        Rect bounds = new Rect(slot.left, slot.top, slot.right(), slot.bottom());
        if (snapshot.isFreeform() && snapshot.bounds.equals(bounds)) {
            finish(task, request);
            return;
        }
        WindowTransactions.WindowingFix fix = snapshot.isFreeform()
                ? WindowTransactions.WindowingFix.KEEP_MODE : WindowTransactions.WindowingFix.MAKE_FREEFORM;
        if (applySync(task, request, bounds, fix)) {
            return;
        }
        if (!applyPlain(task, bounds, fix) && !applyLegacy(task, bounds)) {
            AgentLog.warn("could not resize " + request + "; reporting its slot so the host does not wait");
        }
        finish(task, request);
    }

    private boolean applySync(TaskState task, SizeRequest request, Rect bounds, WindowTransactions.WindowingFix fix) {
        try {
            Object transaction = WindowTransactions.bounds(task.snapshot.token, bounds, fix);
            WindowTransactions.applySync(transaction,
                    new SyncCallback(ready -> loop.execute(() -> onSynced(task, request, ready))));
            loop.schedule(() -> onSyncTimeout(task, request), SYNC_TIMEOUT_MILLIS);
            return true;
        } catch (HiddenApiException | LinkageError e) {
            AgentLog.warnOnce("sync-resize", "applySyncTransaction failed, using a plain transaction: " + e);
            return false;
        }
    }

    private static boolean applyPlain(TaskState task, Rect bounds, WindowTransactions.WindowingFix fix) {
        try {
            WindowTransactions.apply(WindowTransactions.bounds(task.snapshot.token, bounds, fix));
            return true;
        } catch (HiddenApiException e) {
            AgentLog.warnOnce("plain-resize", "applyTransaction failed, using resizeTask: " + e);
            return false;
        }
    }

    private static boolean applyLegacy(TaskState task, Rect bounds) {
        try {
            return (Boolean) Hidden.call(Services.activityTaskManager(), "resizeTask",
                    Hidden.types(int.class, Rect.class, int.class), task.id, bounds, RESIZE_MODE_PRESERVE_WINDOW);
        } catch (HiddenApiException e) {
            AgentLog.warn("resizeTask " + task.id + " failed: " + e.getMessage());
            return false;
        }
    }

    private void onSynced(TaskState task, SizeRequest request, SurfaceControl.Transaction transaction) {
        boolean current = coalescer.isInFlight(request);
        if (current && task.leash != null) {
            Leashes.place(transaction, task.leash, task.slot);
            task.placedLeash = task.slot;
        }
        transaction.apply();
        transaction.close();
        if (current) {
            finish(task, request);
        }
    }

    private void onSyncTimeout(TaskState task, SizeRequest request) {
        if (coalescer.isInFlight(request)) {
            AgentLog.warn("no redraw for " + request + " within " + SYNC_TIMEOUT_MILLIS + " ms");
            finish(task, request);
        }
    }

    private void finish(TaskState task, SizeRequest request) {
        if (task.leash != null && !task.slot.equals(task.placedLeash)) {
            Leashes.move(task.leash, task.slot);
            task.placedLeash = task.slot;
        }
        task.appliedSerial = request.serial;
        listener.onSlotApplied(task);
        SizeRequest next = coalescer.complete(request);
        if (next != null) {
            apply(task, next);
        }
    }
}
