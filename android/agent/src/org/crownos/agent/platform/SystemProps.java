package org.crownos.agent.platform;

/** System properties through the hidden {@code android.os.SystemProperties}. */
public final class SystemProps {
    private static final String CLASS = "android.os.SystemProperties";

    private SystemProps() {
    }

    public static String get(String key, String fallback) {
        return (String) Hidden.callStatic(Hidden.type(CLASS), "get", Hidden.types(String.class, String.class), key,
                fallback);
    }

    public static int getInt(String key, int fallback) {
        try {
            return Integer.parseInt(get(key, "").trim());
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    public static long getLong(String key, long fallback) {
        try {
            return Long.parseLong(get(key, "").trim());
        } catch (NumberFormatException e) {
            return fallback;
        }
    }

    public static boolean isOn(String key) {
        return "1".equals(get(key, "0").trim());
    }

    public static void set(String key, String value) {
        Hidden.callStatic(Hidden.type(CLASS), "set", Hidden.types(String.class, String.class), key, value);
    }
}
