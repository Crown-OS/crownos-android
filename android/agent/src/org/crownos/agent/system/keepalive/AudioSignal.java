package org.crownos.agent.system.keepalive;

import java.util.List;

import org.crownos.agent.keepalive.KeepAliveReasons;
import org.crownos.agent.keepalive.KeepAliveSignal;
import org.crownos.agent.platform.Hidden;
import org.crownos.agent.platform.Services;

/** Some player is playing: an active AudioPlaybackConfiguration. */
final class AudioSignal implements KeepAliveSignal {
    @Override
    public int reason() {
        return KeepAliveReasons.AUDIO;
    }

    @Override
    public String name() {
        return "audio";
    }

    @Override
    public boolean isActive() {
        List<?> players = (List<?>) Hidden.call(Services.audioService(), "getActivePlaybackConfigurations",
                Hidden.NO_TYPES);
        for (Object player : players) {
            if (Boolean.TRUE.equals(Hidden.call(player, "isActive", Hidden.NO_TYPES))) {
                return true;
            }
        }
        return false;
    }
}
