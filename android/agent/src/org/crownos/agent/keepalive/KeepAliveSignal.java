package org.crownos.agent.keepalive;

/** One reason Android may need to keep running with no window open. */
public interface KeepAliveSignal {
    /** The {@link KeepAliveReasons} bit this signal sets. */
    int reason();

    String name();

    boolean isActive() throws Exception;
}
