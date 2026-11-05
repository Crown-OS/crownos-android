package org.crownos.agent.policy;

import java.nio.charset.StandardCharsets;

/** Which files the daemon may ask the agent to install: regular names directly inside the inbox. */
public final class InboxPolicy {
    /** The drop directory crown-android copies APKs into. */
    public static final String INBOX = "/data/crownos/inbox/";
    public static final int MAX_NAME_BYTES = 255;
    private static final char FIRST_PRINTABLE = 0x20;
    private static final char DELETE = 0x7F;

    private InboxPolicy() {
    }

    /**
     * Returns the bare file name that {@code path} names directly under {@link #INBOX}, accepting
     * either that bare name or the absolute path. Returns {@code null} for anything else.
     */
    public static String fileName(String path) {
        if (path == null) {
            return null;
        }
        String name = path.startsWith(INBOX) ? path.substring(INBOX.length()) : path;
        if (name.isEmpty() || name.equals(".") || name.equals("..")
                || name.getBytes(StandardCharsets.UTF_8).length > MAX_NAME_BYTES) {
            return null;
        }
        for (int index = 0; index < name.length(); index++) {
            char character = name.charAt(index);
            if (character == '/' || character < FIRST_PRINTABLE || character == DELETE) {
                return null;
            }
        }
        return name;
    }
}
