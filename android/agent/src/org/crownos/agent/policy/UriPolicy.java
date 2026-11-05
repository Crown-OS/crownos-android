package org.crownos.agent.policy;

import java.net.URI;
import java.net.URISyntaxException;
import java.util.Locale;

/** Which links the daemon may open in Android: well-formed {@code market:} and {@code https:} URIs only. */
public final class UriPolicy {
    public static final int MAX_LENGTH = 8192;
    private static final String MARKET = "market";
    private static final String HTTPS = "https";

    private UriPolicy() {
    }

    public static boolean isAllowed(String uri) {
        if (uri == null || uri.isEmpty() || uri.length() > MAX_LENGTH) {
            return false;
        }
        URI parsed;
        try {
            parsed = new URI(uri);
        } catch (URISyntaxException e) {
            return false;
        }
        String scheme = parsed.getScheme();
        if (scheme == null) {
            return false;
        }
        switch (scheme.toLowerCase(Locale.ROOT)) {
            case HTTPS:
                return !isBlank(parsed.getRawAuthority());
            case MARKET:
                return !isBlank(parsed.getRawSchemeSpecificPart());
            default:
                return false;
        }
    }

    private static boolean isBlank(String value) {
        return value == null || value.isEmpty();
    }
}
