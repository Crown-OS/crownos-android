package org.crownos.agent.system;

import android.os.Bundle;

import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.ProviderConnection;
import org.crownos.agent.util.AgentLog;

/** Writes Settings tables through the settings provider's call interface, as {@code settings put} does. */
final class SettingsWriter implements AutoCloseable {
    /** A Settings table and its provider call suffix. */
    enum Table {
        GLOBAL("global"),
        SYSTEM("system");

        final String suffix;

        Table(String suffix) {
            this.suffix = suffix;
        }
    }

    private static final String AUTHORITY = "settings";
    private static final String VALUE = "value";

    private final ProviderConnection provider;

    private SettingsWriter(ProviderConnection provider) {
        this.provider = provider;
    }

    static SettingsWriter open() {
        ProviderConnection provider = ProviderConnection.open(AUTHORITY);
        if (provider == null) {
            throw new HiddenApiException("the settings provider is not available", null);
        }
        return new SettingsWriter(provider);
    }

    /** Sets {@code name}, skipping the write when it already holds {@code value}. */
    void put(Table table, String name, String value) {
        try {
            Bundle current = provider.call("GET_" + table.suffix, name, null);
            if (current != null && value.equals(current.getString(VALUE))) {
                return;
            }
            Bundle extras = new Bundle();
            extras.putString(VALUE, value);
            provider.call("PUT_" + table.suffix, name, extras);
            AgentLog.info("set " + table.suffix + " " + name + " to " + value);
        } catch (HiddenApiException e) {
            AgentLog.warn("cannot set " + table.suffix + " " + name + ": " + e.getMessage());
        }
    }

    @Override
    public void close() {
        provider.close();
    }
}
