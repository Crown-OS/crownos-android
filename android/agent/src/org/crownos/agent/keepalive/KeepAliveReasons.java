package org.crownos.agent.keepalive;

/** The {@code KeepAlive.reasons} bits of agent-wire v1. */
public final class KeepAliveReasons {
    public static final int AUDIO = 1;
    public static final int CALL = 1 << 1;
    public static final int INSTALL = 1 << 2;
    public static final int DOWNLOAD = 1 << 3;
    public static final int FOREGROUND_SERVICE = 1 << 4;

    private KeepAliveReasons() {
    }
}
