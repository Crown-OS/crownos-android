package org.crownos.agent.task;

import android.graphics.Rect;
import android.view.SurfaceControl;

import org.crownos.agent.atlas.Slot;
import org.crownos.agent.wire.HwcEvent;

/** What the agent tracks for one task. Touched only on the agent loop. */
final class TaskState {
    final int id;
    TaskSnapshot snapshot;
    SurfaceControl leash;
    /** Where the leash was last placed, so unchanged placements cost no transaction. */
    Slot placedLeash;
    /** The atlas slot of an app task, {@code null} until it becomes a window. */
    Slot slot;
    int appliedSerial;
    /** Bounds Android kept after the agent once corrected them; accepted the second time. */
    Rect refusedBounds;
    boolean announced;
    HwcEvent.TaskInfo sentInfo;

    TaskState(int id) {
        this.id = id;
    }

    boolean isWindow() {
        return slot != null;
    }

    /** Forgets everything about the host window; the task itself stays. */
    void closeWindow() {
        slot = null;
        placedLeash = null;
        appliedSerial = 0;
        refusedBounds = null;
        announced = false;
        sentInfo = null;
    }
}
