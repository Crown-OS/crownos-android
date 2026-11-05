package org.crownos.agent.system;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;

import org.crownos.agent.platform.HiddenApiException;
import org.crownos.agent.platform.ProviderConnection;
import org.crownos.agent.util.AgentLog;

/**
 * The Google services framework id ({@code android_id} in gservices), the number to register at
 * google.com/android/uncertified. Read through the Gservices provider as root rather than its
 * database, so the agent never creates root-owned journal files in GSF's data.
 */
public final class GsfId {
    private static final String AUTHORITY = "com.google.android.gsf.gservices";
    private static final Uri GSERVICES = Uri.parse("content://" + AUTHORITY);
    private static final String ANDROID_ID = "android_id";
    private static final int VALUE_COLUMN = 1;
    private static final String NOT_CHECKED_IN = "0";

    private GsfId() {
    }

    /** The id, or an empty string on images without Google services or before the first check-in. */
    public static String read() {
        try (ProviderConnection provider = ProviderConnection.open(AUTHORITY)) {
            if (provider == null) {
                return "";
            }
            Bundle arguments = new Bundle();
            arguments.putStringArray(ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS, new String[] {ANDROID_ID});
            try (Cursor cursor = provider.query(GSERVICES, null, arguments)) {
                if (cursor == null || !cursor.moveToFirst() || cursor.getColumnCount() <= VALUE_COLUMN) {
                    return "";
                }
                String id = cursor.getString(VALUE_COLUMN);
                return id == null || id.trim().equals(NOT_CHECKED_IN) ? "" : id.trim();
            }
        } catch (HiddenApiException | IllegalStateException | SecurityException e) {
            AgentLog.warn("cannot read the GSF id: " + e.getMessage());
            return "";
        }
    }
}
