package org.crownos.agent.wire;

/** Records the agent sends to the hwcomposer. */
public abstract class HwcEvent extends Record {
    HwcEvent() {
    }

    /** Facts about a task, sent when it opens and when they change. */
    public static final class TaskInfo extends HwcEvent {
        public static final int TYPE = 0x0201;

        public final int task;
        public final String packageName;
        public final String label;
        /** {@code 0xAARRGGBB}; 0 means no color. */
        public final int color;
        public final boolean resizable;
        public final int minWidth;
        public final int minHeight;

        public TaskInfo(int task, String packageName, String label, int color, boolean resizable, int minWidth,
                int minHeight) {
            this.task = task;
            this.packageName = packageName;
            this.label = label;
            this.color = color;
            this.resizable = resizable;
            this.minWidth = minWidth;
            this.minHeight = minHeight;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.u32(task).str(packageName).str(label).u32(color).bool(resizable).u32(minWidth).u32(minHeight);
        }

        static TaskInfo read(WireReader in) throws WireException {
            return new TaskInfo(in.u32(), in.str(), in.str(), in.u32(), in.bool(), in.u32(), in.u32());
        }

        /** Whether {@code other} carries the same facts. */
        public boolean sameAs(TaskInfo other) {
            return other != null && task == other.task && packageName.equals(other.packageName)
                    && label.equals(other.label) && color == other.color && resizable == other.resizable
                    && minWidth == other.minWidth && minHeight == other.minHeight;
        }
    }

    /** {@code task} now occupies {@code left, top, right, bottom}, after the SetSize with {@code serial}. */
    public static final class SlotApplied extends HwcEvent {
        public static final int TYPE = 0x0202;

        public final int task;
        public final int serial;
        public final int left;
        public final int top;
        public final int right;
        public final int bottom;

        public SlotApplied(int task, int serial, int left, int top, int right, int bottom) {
            this.task = task;
            this.serial = serial;
            this.left = left;
            this.top = top;
            this.right = right;
            this.bottom = bottom;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) {
            out.u32(task).u32(serial).u32(left).u32(top).u32(right).u32(bottom);
        }

        static SlotApplied read(WireReader in) throws WireException {
            return new SlotApplied(in.u32(), in.u32(), in.u32(), in.u32(), in.u32(), in.u32());
        }
    }

    /** {@code task} is gone. */
    public static final class TaskRemoved extends HwcEvent {
        public static final int TYPE = 0x0203;

        public final int task;

        public TaskRemoved(int task) {
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

        static TaskRemoved read(WireReader in) throws WireException {
            return new TaskRemoved(in.u32());
        }
    }

    /** Raise the window of {@code task} with this xdg-activation token. */
    public static final class Activate extends HwcEvent {
        public static final int TYPE = 0x0204;

        public final int task;
        public final String token;

        public Activate(int task, String token) {
            this.task = task;
            this.token = token;
        }

        @Override
        public int type() {
            return TYPE;
        }

        @Override
        void writePayload(WireWriter out) throws WireException {
            out.u32(task).str(token);
        }

        static Activate read(WireReader in) throws WireException {
            return new Activate(in.u32(), in.str());
        }
    }
}
