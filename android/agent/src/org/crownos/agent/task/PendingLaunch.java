package org.crownos.agent.task;

import android.os.Binder;
import android.os.IBinder;

import java.util.concurrent.ScheduledFuture;

/** A started activity whose task has not shown up yet. */
final class PendingLaunch {
    final IBinder cookie = new Binder();
    /** The daemon's package, or {@code null} for an OpenUri link, which gets no reply. */
    final String packageName;
    final String activationToken;
    /** Atlas key holding the launch bounds until the task appears. */
    final int reservation;
    ScheduledFuture<?> timeout;

    PendingLaunch(String packageName, String activationToken, int reservation) {
        this.packageName = packageName;
        this.activationToken = activationToken;
        this.reservation = reservation;
    }

    boolean isFor(String taskPackage) {
        return packageName != null && packageName.equals(taskPackage);
    }

    String describe() {
        return packageName == null ? "link" : packageName;
    }
}
