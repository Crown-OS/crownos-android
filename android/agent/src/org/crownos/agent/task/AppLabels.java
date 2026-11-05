package org.crownos.agent.task;

import android.content.pm.ApplicationInfo;
import android.content.res.AssetManager;
import android.content.res.Resources;

import java.util.HashMap;
import java.util.Map;

import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/** Application labels read from each package's own resources, without a Context, cached per package. */
final class AppLabels {
    private final Map<String, String> labels = new HashMap<>();

    String of(String packageName) {
        String label = labels.get(packageName);
        if (label == null) {
            label = load(packageName);
            labels.put(packageName, label);
        }
        return label;
    }

    void forget(String packageName) {
        labels.remove(packageName);
    }

    private static String load(String packageName) {
        try {
            ApplicationInfo application = (ApplicationInfo) Hidden.call(Services.packageManager(),
                    "getApplicationInfo", Hidden.types(String.class, long.class, int.class),
                    packageName, 0L, Services.USER_SYSTEM);
            if (application == null) {
                return packageName;
            }
            if (application.nonLocalizedLabel != null) {
                return application.nonLocalizedLabel.toString();
            }
            return application.labelRes == 0 ? packageName : fromResources(application);
        } catch (HiddenApiException | Resources.NotFoundException e) {
            AgentLog.warn("cannot read the label of " + packageName + ": " + e.getMessage());
            return packageName;
        }
    }

    @SuppressWarnings("deprecation")
    private static String fromResources(ApplicationInfo application) {
        AssetManager assets = (AssetManager) Hidden.construct(AssetManager.class, Hidden.NO_TYPES);
        try {
            Hidden.call(assets, "addAssetPath", Hidden.types(String.class), application.sourceDir);
            Resources system = Resources.getSystem();
            Resources resources = new Resources(assets, system.getDisplayMetrics(), system.getConfiguration());
            return resources.getText(application.labelRes).toString();
        } finally {
            assets.close();
        }
    }
}
