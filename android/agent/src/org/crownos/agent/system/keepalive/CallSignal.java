package org.crownos.agent.system.keepalive;

import android.media.AudioManager;

import org.crownos.agent.keepalive.KeepAliveReasons;
import org.crownos.agent.keepalive.KeepAliveSignal;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.Services;
import org.crownos.agent.util.AgentLog;

/** A phone or VoIP call: an audio mode other than normal, or a call Telecom knows about. */
final class CallSignal implements KeepAliveSignal {
    @Override
    public int reason() {
        return KeepAliveReasons.CALL;
    }

    @Override
    public String name() {
        return "call";
    }

    @Override
    public boolean isActive() {
        int mode = (Integer) Hidden.call(Services.audioService(), "getMode", Hidden.NO_TYPES);
        return mode > AudioManager.MODE_NORMAL || inTelecomCall();
    }

    private static boolean inTelecomCall() {
        Object telecom = Services.telecomService();
        if (telecom == null) {
            return false;
        }
        try {
            return (Boolean) Hidden.call(telecom, "isInCall", Hidden.types(String.class, String.class),
                    Services.ROOT_PACKAGE, null);
        } catch (HiddenApiException e) {
            AgentLog.warnOnce("telecom-in-call", "cannot ask Telecom about calls: " + e.getMessage());
            return false;
        }
    }
}
