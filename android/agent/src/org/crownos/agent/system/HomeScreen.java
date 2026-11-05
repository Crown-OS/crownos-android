package org.crownos.agent.system;

import android.os.Bundle;

import java.lang.reflect.Proxy;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/** Replaces the launcher with the code-free blank HOME, so neither HOME nor Recents runs Launcher3. */
final class HomeScreen {
    static final String BLANK_HOME_PACKAGE = "org.crownos.android.home";
    private static final String LAUNCHER_PACKAGE = "com.android.launcher3";
    private static final String HOME_ROLE = "android.app.role.HOME";
    private static final String RESULT_LISTENER = "android.os.RemoteCallback$OnResultListener";
    private static final int COMPONENT_ENABLED_STATE_DISABLED_USER = 3;
    private static final int NO_FLAGS = 0;
    private static final long ROLE_WAIT_SECONDS = 10;

    private HomeScreen() {
    }

    static void useBlankHome() {
        holdHomeRole();
        disableLauncher();
    }

    /** {@code cmd package set-home-activity} goes through the HOME role on Android 13; so does this. */
    private static void holdHomeRole() {
        CountDownLatch answered = new CountDownLatch(1);
        AtomicBoolean granted = new AtomicBoolean();
        try {
            Class<?> listenerType = Hidden.type(RESULT_LISTENER);
            Object listener = Proxy.newProxyInstance(listenerType.getClassLoader(), new Class<?>[] {listenerType},
                    (proxy, method, arguments) -> {
                        if (method.getDeclaringClass() == Object.class) {
                            return objectMethod(proxy, method.getName(), arguments);
                        }
                        granted.set(arguments != null && arguments[0] instanceof Bundle);
                        answered.countDown();
                        return null;
                    });
            Object callback = Hidden.construct(Hidden.type("android.os.RemoteCallback"), Hidden.types(listenerType),
                    listener);
            Hidden.call(Services.roleManager(), "addRoleHolderAsUser",
                    Hidden.types(String.class, String.class, int.class, int.class, "android.os.RemoteCallback"),
                    HOME_ROLE, BLANK_HOME_PACKAGE, NO_FLAGS, Services.USER_SYSTEM, callback);
            if (!answered.await(ROLE_WAIT_SECONDS, TimeUnit.SECONDS)) {
                AgentLog.warn("the role service did not answer the HOME request");
            } else if (!granted.get()) {
                AgentLog.warn("the role service refused HOME for " + BLANK_HOME_PACKAGE);
            }
        } catch (HiddenApiException | IllegalArgumentException e) {
            AgentLog.warn("cannot make " + BLANK_HOME_PACKAGE + " the HOME app: " + e.getMessage());
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }

    private static void disableLauncher() {
        try {
            Object packageManager = Services.packageManager();
            int state = (Integer) Hidden.call(packageManager, "getApplicationEnabledSetting",
                    Hidden.types(String.class, int.class), LAUNCHER_PACKAGE, Services.USER_SYSTEM);
            if (state == COMPONENT_ENABLED_STATE_DISABLED_USER) {
                return;
            }
            Hidden.call(packageManager, "setApplicationEnabledSetting",
                    Hidden.types(String.class, int.class, int.class, int.class, String.class),
                    LAUNCHER_PACKAGE, COMPONENT_ENABLED_STATE_DISABLED_USER, NO_FLAGS, Services.USER_SYSTEM,
                    Services.ROOT_PACKAGE);
            AgentLog.info("disabled " + LAUNCHER_PACKAGE);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot disable " + LAUNCHER_PACKAGE + ": " + e.getMessage());
        }
    }

    private static Object objectMethod(Object proxy, String name, Object[] arguments) {
        switch (name) {
            case "equals":
                return proxy == arguments[0];
            case "hashCode":
                return System.identityHashCode(proxy);
            default:
                return RESULT_LISTENER;
        }
    }
}
