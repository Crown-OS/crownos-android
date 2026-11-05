package org.crownos.agent;

import java.util.Arrays;

final class Commands {
    static final String USAGE = String.join("\n",
            "usage: CLASSPATH=crown-agent-spike.jar app_process / org.crownos.agent.Spike <command>",
            "  list-tasks",
            "  launch <package|package/activity> <l> <t> <r> <b> [freeform|multiwindow]",
            "  resize-wct <task> <l> <t> <r> <b> [--sync]",
            "  resize-legacy <task> <l> <t> <r> <b>",
            "  focus <task>",
            "  front <task>",
            "  remove <task>",
            "  info <task>",
            "  watch [seconds]",
            "  organize [seconds]",
            "  toast <text>",
            "  serve <path-prefix>");

    private static final int FIRST_EDGE_INDEX = 2;
    private static final int LAUNCH_MODE_INDEX = 6;

    private Commands() {
    }

    static boolean isLongRunning(String command) {
        return command.equals("watch") || command.equals("organize");
    }

    static void run(String[] arguments) {
        if (arguments.length == 0) {
            throw new IllegalArgumentException(USAGE);
        }
        switch (arguments[0]) {
            case "list-tasks":
                TaskQueries.list();
                break;
            case "launch":
                Launch.start(required(arguments, 1), Rects.parse(arguments, FIRST_EDGE_INDEX),
                        arguments.length > LAUNCH_MODE_INDEX ? WindowingMode.parse(arguments[LAUNCH_MODE_INDEX])
                                : WindowingMode.FREEFORM);
                break;
            case "resize-wct":
                Resize.withTransaction(taskId(arguments), Rects.parse(arguments, FIRST_EDGE_INDEX),
                        Arrays.asList(arguments).contains("--sync"));
                break;
            case "resize-legacy":
                Resize.legacy(taskId(arguments), Rects.parse(arguments, FIRST_EDGE_INDEX));
                break;
            case "focus":
                TaskControl.focus(taskId(arguments));
                break;
            case "front":
                TaskControl.moveToFront(taskId(arguments));
                break;
            case "remove":
                TaskControl.remove(taskId(arguments));
                break;
            case "info":
                TaskInfoReport.print(taskId(arguments));
                break;
            case "watch":
                TaskStackWatcher.watch(optionalSeconds(arguments));
                break;
            case "organize":
                LeashOrganizer.organize(optionalSeconds(arguments));
                break;
            case "toast":
                Toaster.show(String.join(" ", Arrays.copyOfRange(arguments, 1, Math.max(1, arguments.length))));
                break;
            case "serve":
                CommandServer.serve(required(arguments, 1));
                break;
            default:
                throw new IllegalArgumentException("unknown command " + arguments[0] + "\n" + USAGE);
        }
    }

    private static String required(String[] arguments, int index) {
        if (arguments.length <= index) {
            throw new IllegalArgumentException("missing argument " + index + "\n" + USAGE);
        }
        return arguments[index];
    }

    private static int taskId(String[] arguments) {
        return Integer.parseInt(required(arguments, 1));
    }

    private static int optionalSeconds(String[] arguments) {
        return arguments.length > 1 ? Integer.parseInt(arguments[1]) : 0;
    }
}
