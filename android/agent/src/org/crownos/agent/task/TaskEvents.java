package org.crownos.agent.task;

import android.app.ActivityManager;
import android.view.SurfaceControl;

/** Task lifecycle callbacks, delivered on the agent loop. */
interface TaskEvents {
    /** {@code leash} is {@code null} when the agent does not organize tasks (safe mode). */
    void onAppeared(ActivityManager.RunningTaskInfo info, SurfaceControl leash);

    void onChanged(ActivityManager.RunningTaskInfo info);

    /** The organizer lost the task: it was removed, or another organizer took it. */
    void onVanished(ActivityManager.RunningTaskInfo info);
}
