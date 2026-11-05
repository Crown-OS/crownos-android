package android.window;

import android.view.SurfaceControl;

public abstract class WindowContainerTransactionCallback {
    public WindowContainerTransactionCallback() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public abstract void onTransactionReady(int syncId, SurfaceControl.Transaction transaction);
}
