package org.crownos.agent.system.keepalive;

import android.content.pm.PackageInstaller;

import java.util.List;

import org.crownos.agent.keepalive.KeepAliveReasons;
import org.crownos.agent.keepalive.KeepAliveSignal;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.Services;

/** A PackageInstaller session is making progress, such as a Play Store install. */
final class InstallSignal implements KeepAliveSignal {
    @Override
    public int reason() {
        return KeepAliveReasons.INSTALL;
    }

    @Override
    public String name() {
        return "install";
    }

    @Override
    public boolean isActive() {
        Object installer = Hidden.call(Services.packageManager(), "getPackageInstaller", Hidden.NO_TYPES);
        Object slice = Hidden.call(installer, "getAllSessions", Hidden.types(int.class), Services.USER_SYSTEM);
        for (Object session : (List<?>) Hidden.call(slice, "getList", Hidden.NO_TYPES)) {
            if (((PackageInstaller.SessionInfo) session).isActive()) {
                return true;
            }
        }
        return false;
    }
}
