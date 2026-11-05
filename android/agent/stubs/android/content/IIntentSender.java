package android.content;

import android.os.Bundle;
import android.os.IBinder;
import android.os.RemoteException;

/** Compile-only copy of the hidden Android 13 interface; the device's own is used at runtime. */
public interface IIntentSender extends android.os.IInterface {
    void send(int code, Intent intent, String resolvedType, IBinder whitelistToken, IIntentReceiver finishedReceiver,
            String requiredPermission, Bundle options) throws RemoteException;

    /** Compile-only copy of the AIDL stub. */
    abstract class Stub extends android.os.Binder implements IIntentSender {
        public Stub() {
            throw new UnsupportedOperationException("compile-time stub");
        }

        @Override
        public IBinder asBinder() {
            return this;
        }
    }
}
