package org.crownos.agent.wire;

/** Records the agent sends to crown-androidd. */
public abstract class DaemonEvent extends Record {
    DaemonEvent() {
    }

    /** Android finished booting {@code bootMillis} after its init started. */
    public static final class Ready extends DaemonEvent {
        public static final int TYPE = 0x0401;

        public final int bootMillis;

        public Ready(int bootMillis) {
            this.bootMillis = bootMillis;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(bootMillis);
        }

        static Ready read(WireReader in) throws WireException {
            return new Ready(in.u32());
        }
    }

    /** A task with a window opened, or a Launch resolved to it. */
    public static final class TaskOpened extends DaemonEvent {
        public static final int TYPE = 0x0402;

        public final int task;
        public final String packageName;

        public TaskOpened(int task, String packageName) {
            this.task = task;
            this.packageName = packageName;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.u32(task).str(packageName);
        }

        static TaskOpened read(WireReader in) throws WireException {
            return new TaskOpened(in.u32(), in.str());
        }
    }

    /** A task with a window is gone. */
    public static final class TaskClosed extends DaemonEvent {
        public static final int TYPE = 0x0403;

        public final int task;

        public TaskClosed(int task) {
            this.task = task;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(task);
        }

        static TaskClosed read(WireReader in) throws WireException {
            return new TaskClosed(in.u32());
        }
    }

    /** Every task with a window. */
    public static final class Tasks extends DaemonEvent {
        public static final int TYPE = 0x0404;

        private final int[] tasks;

        public Tasks(int[] tasks) {
            this.tasks = tasks.clone();
        }

        public int[] tasks() {
            return tasks.clone();
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.u32List(tasks);
        }

        static Tasks read(WireReader in) throws WireException {
            return new Tasks(in.u32List());
        }
    }

    /** Why Android must keep running with no window open, as {@code KeepAliveReasons} bits. */
    public static final class KeepAlive extends DaemonEvent {
        public static final int TYPE = 0x0405;

        public final int reasons;

        public KeepAlive(int reasons) {
            this.reasons = reasons;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(reasons);
        }

        static KeepAlive read(WireReader in) throws WireException {
            return new KeepAlive(in.u32());
        }
    }

    /** The Google services framework id, empty when Android has none. */
    public static final class GsfId extends DaemonEvent {
        public static final int TYPE = 0x0406;

        public final String id;

        public GsfId(String id) {
            this.id = id;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(id);
        }

        static GsfId read(WireReader in) throws WireException {
            return new GsfId(in.str());
        }
    }

    /** A Launch did not produce a task. */
    public static final class LaunchFailed extends DaemonEvent {
        public static final int TYPE = 0x0407;

        public final String packageName;
        public final String reason;

        public LaunchFailed(String packageName, String reason) {
            this.packageName = packageName;
            this.reason = reason;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(packageName).str(reason);
        }

        static LaunchFailed read(WireReader in) throws WireException {
            return new LaunchFailed(in.str(), in.str());
        }
    }

    /** The outcome of an InstallPackage. */
    public static final class InstallResult extends DaemonEvent {
        public static final int TYPE = 0x0408;

        public final String path;
        public final boolean ok;
        public final String message;

        public InstallResult(String path, boolean ok, String message) {
            this.path = path;
            this.ok = ok;
            this.message = message;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.str(path).bool(ok).str(message);
        }

        static InstallResult read(WireReader in) throws WireException {
            return new InstallResult(in.str(), in.bool(), in.str());
        }
    }
}
