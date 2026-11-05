package org.crownos.agent.task;

import org.crownos.agent.atlas.Slot;
import org.crownos.agent.wire.DaemonEvent;
import org.crownos.agent.wire.HwcEvent;
import org.crownos.agent.wire.RecordSink;

/** Builds the task records for the hwcomposer and the daemon, sending each fact once. */
final class TaskReporter {
    private final RecordSink hwc;
    private final RecordSink daemon;
    private final AppLabels labels = new AppLabels();

    TaskReporter(RecordSink hwc, RecordSink daemon) {
        this.hwc = hwc;
        this.daemon = daemon;
    }

    /** Sends TaskInfo when the label, color, resizability or minimum size changed. */
    void info(TaskState task) {
        TaskSnapshot snapshot = task.snapshot;
        String label = snapshot.descriptionLabel == null || snapshot.descriptionLabel.isEmpty()
                ? labels.of(snapshot.packageName) : snapshot.descriptionLabel;
        HwcEvent.TaskInfo info = new HwcEvent.TaskInfo(task.id, snapshot.packageName, label, snapshot.color,
                snapshot.resizable, Math.max(0, snapshot.minWidth), Math.max(0, snapshot.minHeight));
        if (!info.sameAs(task.sentInfo)) {
            hwc.send(info);
            task.sentInfo = info;
        }
    }

    void slot(TaskState task, Slot slot) {
        hwc.send(new HwcEvent.SlotApplied(task.id, task.appliedSerial, slot.left, slot.top, slot.right(),
                slot.bottom()));
    }

    /** Tells the hwcomposer everything about a window again, after it reconnected. */
    void resend(TaskState task, Slot slot) {
        task.sentInfo = null;
        info(task);
        slot(task, slot);
    }

    /** TaskOpened, the first time the task becomes a window. */
    void opened(TaskState task) {
        if (!task.announced) {
            task.announced = true;
            daemon.send(new DaemonEvent.TaskOpened(task.id, task.snapshot.packageName));
        }
    }

    /** TaskOpened again, answering a Launch that brought an existing window forward. */
    void reopened(TaskState task) {
        daemon.send(new DaemonEvent.TaskOpened(task.id, task.snapshot.packageName));
    }

    void activate(TaskState task, String token) {
        if (!token.isEmpty()) {
            hwc.send(new HwcEvent.Activate(task.id, token));
        }
    }

    void removed(TaskState task) {
        if (task.isWindow()) {
            hwc.send(new HwcEvent.TaskRemoved(task.id));
        }
        if (task.announced) {
            daemon.send(new DaemonEvent.TaskClosed(task.id));
        }
    }

    void launchFailed(String packageName, String reason) {
        daemon.send(new DaemonEvent.LaunchFailed(packageName, reason));
    }
}
