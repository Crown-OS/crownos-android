package org.crownos.agent;

import android.content.pm.ApplicationInfo;
import android.content.res.AssetManager;
import android.content.res.Configuration;
import android.content.res.Resources;
import android.util.DisplayMetrics;

final class AppLabel {
    private AppLabel() {
    }

    static String of(String packageName) {
        if (packageName == null) {
            return "";
        }
        ApplicationInfo application = (ApplicationInfo) Hidden.call(Services.packageManager(), "getApplicationInfo",
                new Class<?>[] {String.class, long.class, int.class}, packageName, 0L, Services.USER_SYSTEM);
        if (application == null) {
            return packageName;
        }
        if (application.nonLocalizedLabel != null) {
            return application.nonLocalizedLabel.toString();
        }
        if (application.labelRes == 0) {
            return packageName;
        }
        AssetManager assets = (AssetManager) Hidden.construct(AssetManager.class.getName());
        try {
            Hidden.call(assets, "addAssetPath", new Class<?>[] {String.class}, application.sourceDir);
            Resources resources = new Resources(assets, new DisplayMetrics(), new Configuration());
            return resources.getText(application.labelRes).toString();
        } finally {
            assets.close();
        }
    }
}
