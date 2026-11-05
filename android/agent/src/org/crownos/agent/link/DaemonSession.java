package org.crownos.agent.link;

import org.crownos.agent.keepalive.KeepAliveProbe;
import org.crownos.agent.policy.InboxPolicy;
import org.crownos.agent.policy.UriPolicy;
import org.crownos.agent.system.GsfId;
import org.crownos.agent.system.InstallOutcome;
import org.crownos.agent.system.PackageInstall;
import org.crownos.agent.system.ShutdownPreparation;
import org.crownos.agent.task.TaskCoordinator;
import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;
import org.crownos.agent.wire.DaemonCommand;
import org.crownos.agent.wire.DaemonEvent;
import org.crownos.agent.wire.Record;
import org.crownos.agent.wire.RecordSink;

/**
 * The crown-androidd side of {@code /data/crownos/agent.sock}. Slow queries run on their own threads
 * so they never hold up task handling: probes on one, installs on another.
 */
public final class DaemonSession implements PeerLink.Peer, DaemonCommand.Handler {
    private final TaskCoordinator coordinator;
    private final RecordSink daemon;
    private final KeepAliveProbe keepAlive;
    private final Loop probes = new Loop("crown-agent-probe");
    private final Loop installs = new Loop("crown-agent-install");
    private volatile boolean ready;
    private int bootMillis;

    public DaemonSession(TaskCoordinator coordinator, RecordSink daemon, KeepAliveProbe keepAlive) {
        this.coordinator = coordinator;
        this.daemon = daemon;
        this.keepAlive = keepAlive;
    }

    /** Android booted and tasks are organized; tells the daemon now and on every reconnect. */
    public void markReady(int millisSinceBoot) {
        bootMillis = millisSinceBoot;
        ready = true;
        AgentLog.info("ready " + millisSinceBoot + " ms after Android's init started");
        daemon.send(new DaemonEvent.Ready(millisSinceBoot));
    }

    @Override
    public void onConnected() {
        if (ready) {
            daemon.send(new DaemonEvent.Ready(bootMillis));
        }
    }

    @Override
    public boolean onRecord(Record record) {
        if (!(record instanceof DaemonCommand)) {
            return false;
        }
        ((DaemonCommand) record).dispatch(this);
        return true;
    }

    @Override
    public void onLaunch(DaemonCommand.Launch command) {
        if (!ready) {
            daemon.send(new DaemonEvent.LaunchFailed(command.packageName, "Android is still booting"));
            return;
        }
        coordinator.launch(command.packageName, command.width, command.height, command.token);
    }

    @Override
    public void onListTasks() {
        daemon.send(new DaemonEvent.Tasks(coordinator.windowTasks()));
    }

    @Override
    public void onQueryKeepAlive() {
        probes.execute(() -> daemon.send(new DaemonEvent.KeepAlive(ready ? keepAlive.reasons() : 0)));
    }

    @Override
    public void onQueryGsfId() {
        probes.execute(() -> daemon.send(new DaemonEvent.GsfId(ready ? GsfId.read() : "")));
    }

    @Override
    public void onShutdown() {
        AgentLog.info("the daemon is stopping Android");
        probes.execute(ShutdownPreparation::run);
    }

    @Override
    public void onOpenUri(DaemonCommand.OpenUri command) {
        if (!UriPolicy.isAllowed(command.uri)) {
            AgentLog.warn("refused to open a link that is not a market: or https: URI");
            return;
        }
        if (!ready) {
            AgentLog.warn("cannot open a link while Android is still booting");
            return;
        }
        coordinator.openUri(command.uri);
    }

    @Override
    public void onInstallPackage(DaemonCommand.InstallPackage command) {
        String fileName = InboxPolicy.fileName(command.path);
        if (fileName == null) {
            daemon.send(new DaemonEvent.InstallResult(command.path, false,
                    "the path must name a file directly under " + InboxPolicy.INBOX));
            return;
        }
        installs.execute(() -> {
            InstallOutcome outcome = PackageInstall.install(InboxPolicy.INBOX + fileName);
            daemon.send(new DaemonEvent.InstallResult(command.path, outcome.ok, outcome.message));
        });
    }
}
