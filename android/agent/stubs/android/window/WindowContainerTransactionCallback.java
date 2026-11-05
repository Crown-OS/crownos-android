package android.window;

import android.view.SurfaceControl;

/** Compile-only copy of the hidden Android 13 class; the device's own class is used at runtime. */
public abstract class WindowContainerTransactionCallback {
    public WindowContainerTransactionCallback() {
        throw new UnsupportedOperationException("compile-time stub");
    }

    public abstract void onTransactionReady(int syncId, SurfaceControl.Transaction transaction);
}
