package org.crownos.agent.keepalive;

import java.util.List;

/** Combines keep-alive signals. A signal that fails counts as inactive and is reported once per call. */
public final class KeepAliveProbe {
    /** Hears about a signal that could not be read. */
    public interface FailureLog {
        void failed(KeepAliveSignal signal, Throwable failure);
    }

    private final List<KeepAliveSignal> signals;
    private final FailureLog failureLog;

    public KeepAliveProbe(List<KeepAliveSignal> signals, FailureLog failureLog) {
        this.signals = signals;
        this.failureLog = failureLog;
    }

    public int reasons() {
        int reasons = 0;
        for (KeepAliveSignal signal : signals) {
            try {
                if (signal.isActive()) {
                    reasons |= signal.reason();
                }
            } catch (Exception | LinkageError failure) {
                failureLog.failed(signal, failure);
            }
        }
        return reasons;
    }
}
