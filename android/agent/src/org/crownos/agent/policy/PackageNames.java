package org.crownos.agent.policy;

import java.util.regex.Pattern;

/** Android package names as the package parser accepts them: two or more dotted Java-style segments. */
public final class PackageNames {
    public static final int MAX_LENGTH = 255;
    private static final Pattern VALID = Pattern.compile("[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+");

    private PackageNames() {
    }

    public static boolean isValid(String packageName) {
        return packageName != null && packageName.length() <= MAX_LENGTH && VALID.matcher(packageName).matches();
    }
}
