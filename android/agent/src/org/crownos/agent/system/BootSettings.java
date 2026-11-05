package org.crownos.agent.system;

import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.SystemProps;
import org.crownos.agent.util.AgentLog;

/** The settings CrownOS applies on every boot once Android is up. Each is idempotent. */
public final class BootSettings {
    /** Makes the stock HWC mode selection pick multi-window (contract 11). */
    private static final String ACTIVE_APPS = "crownos";
    /** Waydroid patch 0029: zero-height status and navigation bars with no insets. */
    private static final String IMMERSIVE_EVERYWHERE = "immersive.full=*";
    /** AC, USB, wireless and dock: the screen never sleeps while plugged in. */
    private static final String STAY_ON_ALWAYS = "15";
    private static final String NEVER = String.valueOf(Integer.MAX_VALUE);
    private static final String ON = "1";

    private BootSettings() {
    }

    /** Applied even in safe mode. */
    public static void applyActiveApps() {
        try {
            SystemProps.set("waydroid.active_apps", ACTIVE_APPS);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot set waydroid.active_apps: " + e.getMessage());
        }
    }

    /** Skipped in safe mode. */
    public static void apply() {
        try (SettingsWriter settings = SettingsWriter.open()) {
            settings.put(SettingsWriter.Table.GLOBAL, "policy_control", IMMERSIVE_EVERYWHERE);
            settings.put(SettingsWriter.Table.GLOBAL, "render_shadows_in_compositor", ON);
            settings.put(SettingsWriter.Table.GLOBAL, "enable_non_resizable_multi_window", ON);
            settings.put(SettingsWriter.Table.GLOBAL, "stay_on_while_plugged_in", STAY_ON_ALWAYS);
            settings.put(SettingsWriter.Table.SYSTEM, "screen_off_timeout", NEVER);
        } catch (HiddenApiException e) {
            AgentLog.warn("boot settings not applied: " + e.getMessage());
        }
        HomeScreen.useBlankHome();
    }
}
