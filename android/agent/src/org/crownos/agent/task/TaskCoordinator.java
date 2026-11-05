package org.crownos.agent.task;

import android.app.ActivityManager;
import android.graphics.Rect;
import android.view.SurfaceControl;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

import org.crownos.agent.atlas.Atlas;
import org.crownos.agent.atlas.Slot;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Dimensions;
import org.crownos.agent.util.Loop;
import org.crownos.agent.wire.RecordSink;

/**
 * Owns every task the agent knows. App tasks become windows with an atlas slot; the coordinator
 * keeps their bounds, leash and FREEFORM mode on that slot and tells the hwcomposer and the daemon.
 * Everything runs on the agent loop.
 */
public final class TaskCoordinator implements TaskEvents, TaskResizer.Listener, TaskLauncher.Listener {
    private final int displayWidth;
    private final int displayHeight;
    private final Atlas atlas;
    private final Map<Integer, TaskState> tasks = new HashMap<>();
    private final TaskReporter reporter;
    private final TaskResizer resizer;
    private final TaskLauncher launcher;
    private TaskPlacement placement = TaskPlacement.OBSERVED;
    private Runnable reclaim = () -> { };

    public TaskCoordinator(Loop loop, int displayWidth, int displayHeight, RecordSink hwc, RecordSink daemon) {
        this.displayWidth = displayWidth;
        this.displayHeight = displayHeight;
        this.atlas = new Atlas(displayWidth, displayHeight);
        this.reporter = new TaskReporter(hwc, daemon);
        this.resizer = new TaskResizer(loop, atlas, this);
        this.launcher = new TaskLauncher(loop, atlas, this);
    }

    void use(TaskPlacement newPlacement, Runnable reclaimOrganizer) {
        placement = newPlacement;
        reclaim = reclaimOrganizer;
    }

    @Override
    public void onAppeared(ActivityManager.RunningTaskInfo info, SurfaceControl leash) {
        TaskSnapshot snapshot = TaskSnapshot.of(info);
        TaskState task = tasks.get(snapshot.id);
        if (task == null) {
            task = new TaskState(snapshot.id);
            tasks.put(task.id, task);
        }
        TaskSnapshot previous = task.snapshot;
        Leashes.release(task.leash);
        task.leash = leash;
        task.snapshot = snapshot;
        task.placedLeash = null;
        if (leash != null) {
            Slot where = task.isWindow() ? task.slot : ownBounds(snapshot);
            Leashes.show(leash, where);
            task.placedLeash = where;
        }
        refresh(task, previous);
    }

    @Override
    public void onChanged(ActivityManager.RunningTaskInfo info) {
        TaskState task = tasks.get(info.taskId);
        if (task == null) {
            return;
        }
        TaskSnapshot previous = task.snapshot;
        task.snapshot = TaskSnapshot.of(info);
        refresh(task, previous);
    }

    @Override
    public void onVanished(ActivityManager.RunningTaskInfo info) {
        TaskState task = tasks.get(info.taskId);
        if (task == null) {
            return;
        }
        Leashes.release(task.leash);
        task.leash = null;
        task.placedLeash = null;
        if (stillRunning(task.id)) {
            reclaim.run();
            return;
        }
        forget(task);
    }

    private static boolean stillRunning(int taskId) {
        try {
            return TaskQueries.isRunning(taskId);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot tell whether task " + taskId + " still runs; treating it as removed: "
                    + e.getMessage());
            return false;
        }
    }

    /** The task is gone for certain, as a safe-mode poll found. */
    void onRemoved(int taskId) {
        TaskState task = tasks.get(taskId);
        if (task != null) {
            forget(task);
        }
    }

    boolean knows(int taskId) {
        return tasks.containsKey(taskId);
    }

    List<Integer> knownTasks() {
        return new ArrayList<>(tasks.keySet());
    }

    boolean hasLeash(int taskId) {
        TaskState task = tasks.get(taskId);
        return task != null && task.leash != null;
    }

    /** Every task that is a window, as announced to the daemon. */
    public int[] windowTasks() {
        int[] ids = new int[tasks.size()];
        int count = 0;
        for (TaskState task : tasks.values()) {
            if (task.announced) {
                ids[count++] = task.id;
            }
        }
        return Arrays.copyOf(ids, count);
    }

    /** Sends the hwcomposer every window's facts and slot, after it connected. */
    public void resendWindows() {
        for (TaskState task : tasks.values()) {
            if (task.isWindow()) {
                reporter.resend(task, task.placedLeash != null ? task.placedLeash : task.slot);
            }
        }
    }

    public void launch(String packageName, int width, int height, String activationToken) {
        launcher.launch(packageName, Dimensions.fromWire(width, displayWidth),
                Dimensions.fromWire(height, displayHeight), activationToken);
    }

    public void openUri(String uri) {
        launcher.openUri(uri);
    }

    public void resize(int taskId, int width, int height, int serial) {
        TaskState task = window(taskId);
        if (task == null) {
            AgentLog.warn("SetSize for task " + taskId + ", which is not a window");
            return;
        }
        resizer.request(task, Math.max(Dimensions.fromWire(width, displayWidth), task.snapshot.minWidth),
                Math.max(Dimensions.fromWire(height, displayHeight), task.snapshot.minHeight), serial);
    }

    public void focus(int taskId) {
        if (window(taskId) != null) {
            atlas.focus(taskId);
            TaskControl.focus(taskId);
        }
    }

    public void close(int taskId) {
        if (window(taskId) != null) {
            TaskControl.remove(taskId);
        }
    }

    public void back(int taskId) {
        TaskState task = window(taskId);
        if (task == null) {
            return;
        }
        if (!task.snapshot.focused) {
            TaskControl.focus(taskId);
        }
        TaskControl.back(task.snapshot.displayId);
    }

    public void setHostVisible(int taskId, boolean visible) {
        if (window(taskId) != null) {
            atlas.setVisible(taskId, visible);
        }
    }

    @Override
    public void onSlotApplied(TaskState task) {
        reporter.slot(task, task.slot);
        if (atlas.isOverflowed(task.id)) {
            raiseMostRecentlyFocused();
        }
    }

    @Override
    public boolean onLaunchResolved(PendingLaunch launch, int taskId) {
        TaskState task = window(taskId);
        if (task == null) {
            return false;
        }
        finishLaunch(launch, task);
        return true;
    }

    @Override
    public void onLaunchFailed(PendingLaunch launch, String reason) {
        releaseSlot(launch.reservation);
        AgentLog.warn("launch of " + launch.describe() + " failed: " + reason);
        if (launch.packageName != null) {
            reporter.launchFailed(launch.packageName, reason);
        }
    }

    @Override
    public void onLaunchRefused(String packageName, String reason) {
        AgentLog.warn("launch of " + packageName + " refused: " + reason);
        reporter.launchFailed(packageName, reason);
    }

    private void refresh(TaskState task, TaskSnapshot previous) {
        TaskSnapshot snapshot = task.snapshot;
        if (!snapshot.isApp()) {
            placeLeash(task, ownBounds(snapshot));
            return;
        }
        if (!task.isWindow()) {
            openWindow(task);
            return;
        }
        if (snapshot.focused && (previous == null || !previous.focused)) {
            atlas.focus(task.id);
        }
        reporter.info(task);
        PendingLaunch launch = launcher.claimByCookie(snapshot);
        if (launch != null) {
            finishLaunch(launch, task);
        }
        if (placement == TaskPlacement.OBSERVED || resizer.isBusy(task.id)) {
            return;
        }
        if (!snapshot.isFreeform() && (previous == null || previous.isFreeform())) {
            resizer.request(task, task.slot.width, task.slot.height, task.appliedSerial);
            return;
        }
        reconcile(task);
    }

    private void openWindow(TaskState task) {
        TaskSnapshot snapshot = task.snapshot;
        PendingLaunch launch = launcher.claim(snapshot);
        if (launch != null) {
            atlas.rename(launch.reservation, task.id);
        }
        task.slot = atlas.slotOf(task.id);
        if (task.slot == null) {
            task.slot = atlas.place(task.id, initialWidth(snapshot), initialHeight(snapshot));
        }
        if (snapshot.focused) {
            atlas.focus(task.id);
        }
        reporter.info(task);
        if (launch != null) {
            finishLaunch(launch, task);
        }
        reporter.opened(task);
        if (placement == TaskPlacement.ORGANIZED) {
            resizer.request(task, task.slot.width, task.slot.height, task.appliedSerial);
        } else {
            reporter.slot(task, ownBounds(snapshot));
        }
    }

    /** Follows bounds Android chose by itself, such as a raised minimum size, without fighting it. */
    private void reconcile(TaskState task) {
        Rect bounds = task.snapshot.bounds;
        Slot slot = task.slot;
        if (bounds.left == slot.left && bounds.top == slot.top && slot.sameSize(bounds.width(), bounds.height())) {
            task.refusedBounds = null;
            placeLeash(task, slot);
            return;
        }
        if (bounds.equals(task.refusedBounds)) {
            Slot actual = ownBounds(task.snapshot);
            AgentLog.warnOnce("refused-" + task.id + "-" + bounds, "task " + task.id + " keeps " + actual
                    + " instead of its slot " + slot);
            placeLeash(task, actual);
            reporter.slot(task, actual);
            return;
        }
        task.refusedBounds = new Rect(bounds);
        resizer.request(task, bounds.width(), bounds.height(), task.appliedSerial);
    }

    private void finishLaunch(PendingLaunch launch, TaskState task) {
        if (atlas.slotOf(launch.reservation) != null) {
            releaseSlot(launch.reservation);
        }
        atlas.focus(task.id);
        if (task.announced && launch.packageName != null) {
            reporter.reopened(task);
        }
        reporter.activate(task, launch.activationToken);
    }

    private void forget(TaskState task) {
        tasks.remove(task.id);
        resizer.forget(task.id);
        Leashes.release(task.leash);
        reporter.removed(task);
        if (task.isWindow()) {
            releaseSlot(task.id);
        }
    }

    /** Frees an atlas key and moves overflowed windows into the space it leaves. */
    private void releaseSlot(int key) {
        for (Integer moved : atlas.remove(key)) {
            TaskState task = tasks.get(moved);
            if (task != null && task.isWindow()) {
                resizer.request(task, task.slot.width, task.slot.height, task.appliedSerial);
            }
        }
    }

    private void raiseMostRecentlyFocused() {
        for (Integer key : atlas.stackingOrder()) {
            TaskState task = tasks.get(key);
            if (task != null) {
                TaskControl.raise(task.snapshot.token);
                return;
            }
        }
    }

    private void placeLeash(TaskState task, Slot slot) {
        if (task.leash != null && !slot.equals(task.placedLeash)) {
            Leashes.move(task.leash, slot);
            task.placedLeash = slot;
        }
    }

    private TaskState window(int taskId) {
        TaskState task = tasks.get(taskId);
        return task != null && task.isWindow() ? task : null;
    }

    private Slot ownBounds(TaskSnapshot snapshot) {
        Rect bounds = snapshot.bounds;
        return new Slot(snapshot.positionInParent.x, snapshot.positionInParent.y,
                bounds.isEmpty() ? displayWidth : bounds.width(), bounds.isEmpty() ? displayHeight : bounds.height());
    }

    private boolean hasOwnSize(TaskSnapshot snapshot) {
        Rect bounds = snapshot.bounds;
        return snapshot.isFreeform() && !bounds.isEmpty()
                && (bounds.width() < displayWidth || bounds.height() < displayHeight);
    }

    private int initialWidth(TaskSnapshot snapshot) {
        return hasOwnSize(snapshot) ? snapshot.bounds.width() : launcher.defaultWidth();
    }

    private int initialHeight(TaskSnapshot snapshot) {
        return hasOwnSize(snapshot) ? snapshot.bounds.height() : launcher.defaultHeight();
    }
}
