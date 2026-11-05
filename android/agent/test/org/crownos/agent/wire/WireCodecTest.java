package org.crownos.agent.wire;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

import org.crownos.agent.testing.Check;
import org.crownos.agent.testing.Hex;
import org.crownos.agent.testing.TestRunner;

public final class WireCodecTest {
    private static final int KEEP_ALIVE_AUDIO_AND_DOWNLOAD = 0b1001;

    private WireCodecTest() {
    }

    /** One sample of every record, named and ordered as in the vectors file. */
    private static Map<String, Record> samples() {
        Map<String, Record> samples = new LinkedHashMap<>();
        samples.put("hello", new Hello(1, "2026.11.0"));
        samples.put("set_size", new HwcCommand.SetSize(7, 1280, 800, 42));
        samples.put("focus", new HwcCommand.Focus(7));
        samples.put("close", new HwcCommand.Close(7));
        samples.put("back", new HwcCommand.Back(7));
        samples.put("visibility", new HwcCommand.Visibility(7, false));
        samples.put("task_info", new HwcEvent.TaskInfo(7, "com.whatsapp", "WhatsApp", 0xFF075E54, true, 320, 480));
        samples.put("slot_applied", new HwcEvent.SlotApplied(7, 42, 0, 0, 1280, 800));
        samples.put("task_removed", new HwcEvent.TaskRemoved(7));
        samples.put("activate", new HwcEvent.Activate(7, "tok-1"));
        samples.put("launch", new DaemonCommand.Launch("com.whatsapp", 1280, 800, ""));
        samples.put("list_tasks", new DaemonCommand.ListTasks());
        samples.put("query_keep_alive", new DaemonCommand.QueryKeepAlive());
        samples.put("query_gsf_id", new DaemonCommand.QueryGsfId());
        samples.put("shutdown", new DaemonCommand.Shutdown());
        samples.put("open_uri", new DaemonCommand.OpenUri("market://details?id=com.whatsapp"));
        samples.put("install_package", new DaemonCommand.InstallPackage("app.apk"));
        samples.put("ready", new DaemonEvent.Ready(9500));
        samples.put("task_opened", new DaemonEvent.TaskOpened(7, "com.whatsapp"));
        samples.put("task_closed", new DaemonEvent.TaskClosed(7));
        samples.put("tasks", new DaemonEvent.Tasks(new int[] {7, 9}));
        samples.put("keep_alive", new DaemonEvent.KeepAlive(KEEP_ALIVE_AUDIO_AND_DOWNLOAD));
        samples.put("gsf_id", new DaemonEvent.GsfId("3a9f"));
        samples.put("launch_failed", new DaemonEvent.LaunchFailed("com.example", "not installed"));
        samples.put("install_result", new DaemonEvent.InstallResult("app.apk", true, ""));
        return samples;
    }

    private static List<String[]> vectors(Path file) throws IOException {
        List<String[]> vectors = new ArrayList<>();
        for (String line : Files.readAllLines(file, StandardCharsets.UTF_8)) {
            if (line.trim().isEmpty() || line.startsWith("#")) {
                continue;
            }
            vectors.add(line.split(" ", 2));
        }
        return vectors;
    }

    private static Record decode(byte[] bytes) throws IOException, WireException {
        return new RecordInput(new ByteArrayInputStream(bytes)).read();
    }

    private static WireException.Kind decodeFailure(byte[] bytes) {
        return Check.raises(WireException.class, () -> decode(bytes), "decode " + Hex.encode(bytes)).kind;
    }

    public static void run(TestRunner runner, Path vectorFile) {
        runner.test("wire: every record matches its golden vector", () -> {
            List<String[]> vectors = vectors(vectorFile);
            Map<String, Record> samples = samples();
            Check.equal(samples.size(), vectors.size(), "one vector per record type");
            int index = 0;
            for (Map.Entry<String, Record> sample : samples.entrySet()) {
                String[] vector = vectors.get(index++);
                Check.equal(sample.getKey(), vector[0], "vector order");
                Check.equal(vector[1], Hex.encode(WireCodec.encode(sample.getValue())), sample.getKey());
            }
        });

        runner.test("wire: every vector decodes to its record type and re-encodes to the same bytes", () -> {
            Map<String, Record> samples = samples();
            for (String[] vector : vectors(vectorFile)) {
                byte[] bytes = Hex.decode(vector[1]);
                Record decoded = decode(bytes);
                Check.equal(samples.get(vector[0]).getClass(), decoded.getClass(), vector[0]);
                Check.equal(vector[1], Hex.encode(WireCodec.encode(decoded)), vector[0] + " round trip");
            }
        });

        runner.test("wire: decoded fields keep their values", () -> {
            HwcCommand.SetSize setSize = (HwcCommand.SetSize) decode(WireCodec.encode(
                    new HwcCommand.SetSize(7, 1280, 800, -2)));
            Check.equal(7, setSize.task, "task");
            Check.equal(1280, setSize.width, "width");
            Check.equal(800, setSize.height, "height");
            Check.equal(-2, setSize.serial, "u32 serial keeps its bits");
            DaemonEvent.InstallResult result = (DaemonEvent.InstallResult) decode(WireCodec.encode(
                    new DaemonEvent.InstallResult("a.apk", false, "naïve ✓")));
            Check.equal("naïve ✓", result.message, "utf-8 string");
            Check.isTrue(!result.ok, "bool");
            DaemonEvent.Tasks tasks = (DaemonEvent.Tasks) decode(WireCodec.encode(new DaemonEvent.Tasks(new int[] {3, 1})));
            Check.isTrue(Arrays.equals(new int[] {3, 1}, tasks.tasks()), "list");
        });

        runner.test("wire: records follow each other on one stream", () -> {
            ByteArrayOutputStream stream = new ByteArrayOutputStream();
            stream.write(WireCodec.encode(new Hello(1, "b")));
            stream.write(WireCodec.encode(new HwcCommand.Focus(3)));
            RecordInput input = new RecordInput(new ByteArrayInputStream(stream.toByteArray()));
            Check.isTrue(input.read() instanceof Hello, "first record");
            Check.equal(3, ((HwcCommand.Focus) input.read()).task, "second record");
            Check.equal(null, input.read(), "clean end of stream");
        });

        runner.test("wire: a partial record is an unexpected end of stream", () -> {
            byte[] bytes = WireCodec.encode(new HwcCommand.Focus(1));
            Check.equal(null, decode(new byte[0]), "empty stream");
            for (int cut = 1; cut < bytes.length; cut++) {
                byte[] partial = Arrays.copyOf(bytes, cut);
                Check.raises(EOFException.class, () -> decode(partial), "cut at " + cut);
            }
        });

        runner.test("wire: trailing or missing payload bytes are malformed", () -> {
            byte[] focus = WireCodec.encode(new HwcCommand.Focus(1));
            byte[] longer = Arrays.copyOf(focus, focus.length + 1);
            longer[0]++;
            Check.equal(WireException.Kind.MALFORMED, decodeFailure(longer), "trailing byte");
            Check.equal(WireException.Kind.MALFORMED, decodeFailure(new byte[] {4, 0, 0, 0, 0x02, 0x01, 0, 0}),
                    "missing task");
            Check.equal(WireException.Kind.MALFORMED, decodeFailure(new byte[] {2, 0, 0, 0, 0x02, 0x01}),
                    "length shorter than the header");
        });

        runner.test("wire: unknown types and huge records are rejected", () -> {
            Check.equal(WireException.Kind.UNKNOWN_TYPE,
                    decodeFailure(new byte[] {4, 0, 0, 0, (byte) 0xFF, 0x7F, 0, 0}), "unknown type");
            Check.equal(WireException.Kind.TOO_LONG, decodeFailure(new byte[] {0, 0, 2, 0}), "131072 bytes");
            Check.equal(WireException.Kind.TOO_LONG,
                    decodeFailure(new byte[] {(byte) 0xFF, (byte) 0xFF, (byte) 0xFF, (byte) 0xFF}), "4 GiB");
        });

        runner.test("wire: booleans other than zero or one are malformed", () -> {
            byte[] bytes = WireCodec.encode(new HwcCommand.Visibility(1, true));
            bytes[bytes.length - 4] = 2;
            Check.equal(WireException.Kind.MALFORMED, decodeFailure(bytes), "visible = 2");
        });

        runner.test("wire: invalid utf-8 is malformed", () -> {
            byte[] bytes = WireCodec.encode(new DaemonCommand.OpenUri("ab"));
            bytes[bytes.length - 1] = (byte) 0xC3;
            Check.equal(WireException.Kind.MALFORMED, decodeFailure(bytes), "truncated sequence");
        });

        runner.test("wire: oversized fields are refused on encode", () -> {
            char[] text = new char[0x10000];
            Arrays.fill(text, 'a');
            Check.equal(WireException.Kind.STRING_TOO_LONG, Check.raises(WireException.class,
                    () -> WireCodec.encode(new DaemonCommand.OpenUri(new String(text))), "65536 byte string").kind,
                    "string");
            Check.equal(WireException.Kind.LIST_TOO_LONG, Check.raises(WireException.class,
                    () -> WireCodec.encode(new DaemonEvent.Tasks(new int[0x10000])), "65536 items").kind, "list");
            char[] nearLimit = new char[0xFFF0];
            Arrays.fill(nearLimit, 'a');
            String longString = new String(nearLimit);
            Check.equal(WireException.Kind.TOO_LONG, Check.raises(WireException.class,
                    () -> WireCodec.encode(new DaemonEvent.LaunchFailed(longString, longString)), "two long strings")
                    .kind, "record");
        });
    }
}
