package org.crownos.agent.platform;

import android.content.AttributionSource;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.IBinder;
import android.os.Process;

/**
 * A content provider reached without a Context, the way the {@code content} shell command does:
 * {@code IActivityManager.getContentProviderExternal}, then the provider's binder directly.
 */
public final class ProviderConnection implements AutoCloseable {
    private static final String TAG = "crown-agent";

    private final String authority;
    private final IBinder token;
    private final Object provider;
    private final AttributionSource attribution =
            new AttributionSource.Builder(Process.myUid()).setPackageName(Services.ROOT_PACKAGE).build();

    private ProviderConnection(String authority, IBinder token, Object provider) {
        this.authority = authority;
        this.token = token;
        this.provider = provider;
    }

    /** Opens {@code authority}, or returns {@code null} when no installed package provides it. */
    public static ProviderConnection open(String authority) {
        IBinder token = new Binder();
        Object holder = Hidden.call(Services.activityManager(), "getContentProviderExternal",
                Hidden.types(String.class, int.class, IBinder.class, String.class),
                authority, Services.USER_SYSTEM, token, TAG);
        Object provider = holder == null ? null : Hidden.field(holder, "provider");
        return provider == null ? null : new ProviderConnection(authority, token, provider);
    }

    public Bundle call(String method, String argument, Bundle extras) {
        return (Bundle) Hidden.call(provider, "call",
                Hidden.types(AttributionSource.class, String.class, String.class, String.class, Bundle.class),
                attribution, authority, method, argument, extras);
    }

    public Cursor query(Uri uri, String[] projection, Bundle queryArguments) {
        return (Cursor) Hidden.call(provider, "query",
                Hidden.types(AttributionSource.class, Uri.class, String[].class, Bundle.class,
                        "android.os.ICancellationSignal"),
                attribution, uri, projection, queryArguments, null);
    }

    @Override
    public void close() {
        Hidden.call(Services.activityManager(), "removeContentProviderExternalAsUser",
                Hidden.types(String.class, IBinder.class, int.class), authority, token, Services.USER_SYSTEM);
    }
}
