package org.crownos.agent.system.keepalive;

import java.util.Arrays;

import org.crownos.agent.keepalive.KeepAliveProbe;
import org.crownos.agent.util.AgentLog;

/** The keep-alive probe over every Android signal. */
public final class KeepAliveSignals {
    private KeepAliveSignals() {
    }

    public static KeepAliveProbe probe() {
        return new KeepAliveProbe(
                Arrays.asList(new AudioSignal(), new CallSignal(), new InstallSignal(), new DownloadSignal(),
                        new ForegroundServiceSignal()),
                (signal, failure) -> AgentLog.warnOnce("keep-alive-" + signal.name(),
                        "cannot read the " + signal.name() + " keep-alive signal: " + failure));
    }
}
