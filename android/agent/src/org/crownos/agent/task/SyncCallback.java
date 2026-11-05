package org.crownos.agent.task;

import android.view.SurfaceControl;
import android.window.WindowContainerTransactionCallback;

import java.util.function.Consumer;

/** Hands the transaction of a finished sync to the agent loop. Runs on a binder thread. */
final class SyncCallback extends WindowContainerTransactionCallback {
    private final Consumer<SurfaceControl.Transaction> ready;

    SyncCallback(Consumer<SurfaceControl.Transaction> ready) {
        this.ready = ready;
    }

    @Override
    public void onTransactionReady(int syncId, SurfaceControl.Transaction transaction) {
        ready.accept(transaction);
    }
}
