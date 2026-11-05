package org.crownos.agent.system.keepalive;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;

import org.crownos.agent.keepalive.KeepAliveReasons;
import org.crownos.agent.keepalive.KeepAliveSignal;
import org.crownos.agent.platform.ProviderConnection;

/** DownloadManager has a pending or running download. */
final class DownloadSignal implements KeepAliveSignal {
    private static final String AUTHORITY = "downloads";
    private static final Uri ALL_DOWNLOADS = Uri.parse("content://downloads/all_downloads");
    /** {@code Downloads.Impl.STATUS_PENDING} and {@code STATUS_RUNNING}. */
    private static final String UNFINISHED = "status IN (190, 192)";
    private static final String[] ID_ONLY = {"_id"};

    @Override
    public int reason() {
        return KeepAliveReasons.DOWNLOAD;
    }

    @Override
    public String name() {
        return "download";
    }

    @Override
    public boolean isActive() {
        try (ProviderConnection provider = ProviderConnection.open(AUTHORITY)) {
            if (provider == null) {
                return false;
            }
            Bundle arguments = new Bundle();
            arguments.putString(ContentResolver.QUERY_ARG_SQL_SELECTION, UNFINISHED);
            try (Cursor downloads = provider.query(ALL_DOWNLOADS, ID_ONLY, arguments)) {
                return downloads != null && downloads.getCount() > 0;
            }
        }
    }
}
