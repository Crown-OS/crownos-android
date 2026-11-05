package org.crownos.agent.wire;

/** Records the hwcomposer sends to the agent on {@code /dev/socket/crown_agent}. */
public abstract class HwcCommand extends Record {
    /** Receives each command type. */
    public interface Handler {
        void onSetSize(SetSize command);

        void onFocus(Focus command);

        void onClose(Close command);

        void onBack(Back command);

        void onVisibility(Visibility command);
    }

    HwcCommand() {
    }

    public abstract void dispatch(Handler handler);

    /** The host window of {@code task} is now {@code width}x{@code height} pixels. */
    public static final class SetSize extends HwcCommand {
        public static final int TYPE = 0x0101;

        public final int task;
        public final int width;
        public final int height;
        public final int serial;

        public SetSize(int task, int width, int height, int serial) {
            this.task = task;
            this.width = width;
            this.height = height;
            this.serial = serial;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(task).u32(width).u32(height).u32(serial);
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onSetSize(this);
        }

        static SetSize read(WireReader in) throws WireException {
            return new SetSize(in.u32(), in.u32(), in.u32(), in.u32());
        }
    }

    /** The host focused the window of {@code task}. */
    public static final class Focus extends HwcCommand {
        public static final int TYPE = 0x0102;

        public final int task;

        public Focus(int task) {
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

        @Override
        public void dispatch(Handler handler) {
            handler.onFocus(this);
        }

        static Focus read(WireReader in) throws WireException {
            return new Focus(in.u32());
        }
    }

    /** The user closed the window of {@code task}. */
    public static final class Close extends HwcCommand {
        public static final int TYPE = 0x0103;

        public final int task;

        public Close(int task) {
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

        @Override
        public void dispatch(Handler handler) {
            handler.onClose(this);
        }

        static Close read(WireReader in) throws WireException {
            return new Close(in.u32());
        }
    }

    /** The user pressed Back in the window of {@code task}. */
    public static final class Back extends HwcCommand {
        public static final int TYPE = 0x0104;

        public final int task;

        public Back(int task) {
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

        @Override
        public void dispatch(Handler handler) {
            handler.onBack(this);
        }

        static Back read(WireReader in) throws WireException {
            return new Back(in.u32());
        }
    }

    /** Whether the host shows the window of {@code task} at all. */
    public static final class Visibility extends HwcCommand {
        public static final int TYPE = 0x0105;

        public final int task;
        public final boolean visible;

        public Visibility(int task, boolean visible) {
            this.task = task;
            this.visible = visible;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(task).bool(visible);
        }

        @Override
        public void dispatch(Handler handler) {
            handler.onVisibility(this);
        }

        static Visibility read(WireReader in) throws WireException {
            return new Visibility(in.u32(), in.bool());
        }
    }
}
