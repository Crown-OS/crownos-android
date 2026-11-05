package org.crownos.agent.system;

import android.content.IIntentReceiver;
import android.content.IIntentSender;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.os.Bundle;
import android.os.IBinder;

import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/** Receives a PackageInstaller commit status, as {@code pm install} does with its LocalIntentReceiver. */
final class InstallStatusReceiver extends IIntentSender.Stub {
    private final CountDownLatch received = new CountDownLatch(1);
    private volatile Intent status;

    @Override
    public void send(int code, Intent intent, String resolvedType, IBinder whitelistToken,
            IIntentReceiver finishedReceiver, String requiredPermission, Bundle options) {
        status = intent;
        received.countDown();
    }

    InstallOutcome await(long timeoutMinutes) throws InterruptedException {
        if (!received.await(timeoutMinutes, TimeUnit.MINUTES)) {
            return InstallOutcome.failed("the package installer did not answer");
        }
        int code = status.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (code == PackageInstaller.STATUS_SUCCESS) {
            return InstallOutcome.installed();
        }
        if (code == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            return InstallOutcome.failed("the install needs confirmation inside Android");
        }
        String message = status.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE);
        return InstallOutcome.failed(message == null ? "install failed with status " + code : message);
    }
}
