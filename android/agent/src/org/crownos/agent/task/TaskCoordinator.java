package org.crownos.agent.task;

import android.app.ActivityManager;
import android.graphics.Rect;
import android.view.SurfaceControl;

import java.util.List;

import org.crownos.agent.atlas.Atlas;
import org.crownos.agent.atlas.Slot;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;
import org.crownos.agent.wire.RecordSink;

/**
 * Owns every task the agent knows. App tasks become windows with an atlas slot; the coordinator
 * keeps their bounds, leash and FREEFORM mode on that slot and tells the hwcomposer and the daemon.
 * Everything runs on the agent loop.
 */
public final class TaskCoordinator implements TaskEvents, TaskResizer.Listener, TaskLauncher.Listener {
    private final DisplayGeometry display;
    private final Atlas atlas;
    private final TaskTable tasks = new TaskTable();
    private final TaskReporter reporter;
    private final TaskResizer resizer;
    private final TaskLauncher launcher;
    private TaskPlacement placement = TaskPlacement.OBSERVED;
    private Runnable reclaim = () -> { };

    public TaskCoordinator(Loop loop, int displayWidth, int displayHeight, RecordSink hwc, RecordSink daemon) {
        this.display = new DisplayGeometry(displayWidth, displayHeight);
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
        TaskState task = tasks.getOrAdd(snapshot.id);
        TaskSnapshot previous = task.snapshot;
        Leashes.release(task.leash);
        task.leash = leash;
        task.snapshot = snapshot;
        task.placedLeash = null;
        if (leash != null && tasks.isParked(task.id)) {
            Leashes.hide(leash);
        } else if (leash != null) {
            Slot where = task.isWindow() ? task.slot : display.boundsOf(snapshot);
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

    /** The task is gone for certain, as a safe-mode poll found. */
    void onRemoved(int taskId) {
        TaskState task = tasks.get(taskId);
        if (task != null) {
            forget(task);
        }
    }

    boolean knows(int taskId) {
        return tasks.contains(taskId);
    }

    List<Integer> knownTasks() {
        return tasks.ids();
    }

    boolean hasLeash(int taskId) {
        return tasks.hasLeash(taskId);
    }

    /** Every task that is a window, as announced to the daemon. */
    public int[] windowTasks() {
        return tasks.announcedIds();
    }

    /** Sends the hwcomposer every window's facts and slot, after it connected. */
    public void resendWindows() {
        for (TaskState task : tasks.windows()) {
            reporter.resend(task, task.placedLeash != null ? task.placedLeash : task.slot);
        }
    }

    public void launch(String packageName, int width, int height, String activationToken) {
        launcher.launch(packageName, display.wireWidth(width), display.wireHeight(height), activationToken);
    }

    public void openUri(String uri) {
        launcher.openUri(uri);
    }

    public void resize(int taskId, int width, int height, int serial) {
        TaskState task = tasks.window(taskId);
        if (task == null) {
            AgentLog.warn("SetSize for task " + taskId + ", which is not a window");
            return;
        }
        resizer.resize(task, Math.max(display.wireWidth(width), task.snapshot.minWidth),
                Math.max(display.wireHeight(height), task.snapshot.minHeight), serial);
    }

    public void focus(int taskId) {
        tasks.parking().hostFocused(taskId);
        if (tasks.window(taskId) != null) {
            atlas.focus(taskId);
            TaskControl.focus(taskId);
        }
    }

    public void close(int taskId) {
        if (tasks.window(taskId) != null) {
            TaskControl.remove(taskId);
        }
    }

    public void back(int taskId) {
        TaskState task = tasks.window(taskId);
        if (task == null) {
            return;
        }
        if (!task.snapshot.focused) {
            TaskControl.focus(taskId);
        }
        tasks.parking().backPressed(taskId);
        TaskControl.back(task.snapshot.displayId);
    }

    public void setHostVisible(int taskId, boolean visible) {
        if (tasks.window(taskId) != null) {
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
        TaskState task = tasks.get(taskId);
        if (task != null && tasks.parking().cameToFront(taskId)) {
            reopenWindow(task, launch);
            return true;
        }
        if (task == null || !task.isWindow()) {
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
            placeLeash(task, display.boundsOf(snapshot));
            return;
        }
        if (tasks.isParked(task.id)) {
            if (snapshot.cameToFrontSince(previous) && tasks.parking().cameToFront(task.id)) {
                reopenWindow(task, launcher.claim(snapshot));
            }
            return;
        }
        if (!task.isWindow()) {
            openWindow(task, launcher.claim(snapshot));
            return;
        }
        if (snapshot.leftFrontSince(previous) && tasks.parking().leftFront(task.id)) {
            park(task);
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
            resizer.refit(task, task.slot.width, task.slot.height);
            return;
        }
        reconcile(task);
    }

    /** Makes an app task a window; {@code launch} is the Launch that produced it, or {@code null}. */
    private void openWindow(TaskState task, PendingLaunch launch) {
        TaskSnapshot snapshot = task.snapshot;
        if (launch != null) {
            atlas.rename(launch.reservation, task.id);
        }
        task.slot = atlas.slotOf(task.id);
        if (task.slot == null) {
            boolean ownSize = display.hasOwnSize(snapshot);
            task.slot = atlas.place(task.id, ownSize ? snapshot.bounds.width() : launcher.defaultWidth(),
                    ownSize ? snapshot.bounds.height() : launcher.defaultHeight());
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
            resizer.refit(task, task.slot.width, task.slot.height);
        } else {
            reporter.slot(task, display.boundsOf(snapshot));
        }
    }

    /**
     * Back sent the task behind the others: its window closes for the host and the daemon, the app
     * keeps running in Android, and its slot goes back to the atlas.
     */
    private void park(TaskState task) {
        AgentLog.info("Back sent task " + task.id + " behind; closing its window, the app keeps running");
        reporter.removed(task);
        resizer.forget(task.id);
        task.closeWindow();
        if (task.leash != null) {
            Leashes.hide(task.leash);
        }
        releaseSlot(task.id);
    }

    /** A parked task came to the front again: a new window in a fresh slot, from serial 0. */
    private void reopenWindow(TaskState task, PendingLaunch launch) {
        AgentLog.info("parked task " + task.id + " came to the front; it gets a window again");
        openWindow(task, launch);
        if (task.leash != null) {
            Leashes.show(task.leash, task.slot);
            task.placedLeash = task.slot;
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
            Slot actual = display.boundsOf(task.snapshot);
            if (!actual.equals(task.placedLeash)) {
                AgentLog.warn("task " + task.id + " keeps " + actual + " instead of its slot " + slot);
                placeLeash(task, actual);
                reporter.slot(task, actual);
            }
            return;
        }
        task.refusedBounds = new Rect(bounds);
        resizer.refit(task, bounds.width(), bounds.height());
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
                resizer.refit(task, task.slot.width, task.slot.height);
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

    private static boolean stillRunning(int taskId) {
        try {
            return TaskQueries.isRunning(taskId);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot tell whether task " + taskId + " still runs; treating it as removed: "
                    + e.getMessage());
            return false;
        }
    }
}
