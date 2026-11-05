package org.crownos.agent.wire;

/** Records crown-androidd sends to the agent on {@code /data/crownos/agent.sock}. */
public abstract class DaemonCommand extends Record {
    /** Receives each command type. */
    public interface Handler {
        void onLaunch(Launch command);

        void onListTasks();

        void onQueryKeepAlive();

        void onQueryGsfId();

        void onShutdown();

        void onOpenUri(OpenUri command);

        void onInstallPackage(InstallPackage command);
    }

    DaemonCommand() {
    }

    public abstract void dispatch(Handler handler);

    /** Open {@code packageName} in a new task of this size. */
    public static final class Launch extends DaemonCommand {
        public static final int TYPE = 0x0301;

        public final String packageName;
        public final int width;
        public final int height;
        public final String token;

        public Launch(String packageName, int width, int height, String token) {
            this.packageName = packageName;
            this.width = width;
            this.height = height;
            this.token = token;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(packageName).u32(width).u32(height).str(token);
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onLaunch(this);
        }

        static Launch read(WireReader in) throws WireException {
            return new Launch(in.str(), in.u32(), in.u32(), in.str());
        }
    }

    /** Reply with {@link DaemonEvent.Tasks}. */
    public static final class ListTasks extends DaemonCommand {
        public static final int TYPE = 0x0302;

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onListTasks();
        }
    }

    /** Reply with {@link DaemonEvent.KeepAlive}. */
    public static final class QueryKeepAlive extends DaemonCommand {
        public static final int TYPE = 0x0303;

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onQueryKeepAlive();
        }
    }

    /** Reply with {@link DaemonEvent.GsfId}. */
    public static final class QueryGsfId extends DaemonCommand {
        public static final int TYPE = 0x0304;

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onQueryGsfId();
        }
    }

    /** Android is about to stop. */
    public static final class Shutdown extends DaemonCommand {
        public static final int TYPE = 0x0305;

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onShutdown();
        }
    }

    /** Open a {@code market:} or {@code https:} link in Android. */
    public static final class OpenUri extends DaemonCommand {
        public static final int TYPE = 0x0306;

        public final String uri;

        public OpenUri(String uri) {
            this.uri = uri;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(uri);
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onOpenUri(this);
        }

        static OpenUri read(WireReader in) throws WireException {
            return new OpenUri(in.str());
        }
    }

    /** Install the APK at this path under {@code /data/crownos/inbox/}. */
    public static final class InstallPackage extends DaemonCommand {
        public static final int TYPE = 0x0307;

        public final String path;

        public InstallPackage(String path) {
            this.path = path;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(path);
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onInstallPackage(this);
        }

        static InstallPackage read(WireReader in) throws WireException {
            return new InstallPackage(in.str());
        }
    }
}
