package org.crownos.agent.link;

import android.net.Credentials;
import android.net.LocalServerSocket;
import android.net.LocalSocket;
import android.os.SystemClock;

import java.io.IOException;
import java.util.function.IntPredicate;

import org.crownos.agent.util.AgentLog;
import org.crownos.agent.util.Loop;
import org.crownos.agent.wire.Hello;
import org.crownos.agent.wire.Record;
import org.crownos.agent.wire.RecordSink;
import org.crownos.agent.wire.WireCodec;

/**
 * Serves one listening socket: admits peers by SO_PEERCRED uid, keeps the newest connection, sends
 * Hello first and closes a peer whose Hello carries another wire version. Records reach the
 * {@link Peer} on the agent loop.
 */
public final class PeerLink implements RecordSink {
    /** The agent side of one socket. */
    public interface Peer {
        /** The peer's Hello matched; the link now carries records. */
        void onConnected();

        /** Returns {@code false} for a record this socket must not carry, which drops the peer. */
        boolean onRecord(Record record);
    }

    private static final long ACCEPT_RETRY_MILLIS = 1_000;

    private final String name;
    private final LocalServerSocket server;
    private final IntPredicate admitsUid;
    private final String bundle;
    private final Loop loop;
    private Peer peer;
    private volatile Connection current;
    private volatile boolean handshaken;

    private final Connection.Receiver receiver = new Connection.Receiver() {
        @Override
        public void onRecord(Connection connection, Record record) {
            loop.execute(() -> handle(connection, record));
        }

        @Override
        public void onClosed(Connection connection) {
            loop.execute(() -> {
                if (connection == current) {
                    current = null;
                    handshaken = false;
                    AgentLog.info(name + ": peer disconnected");
                }
            });
        }
    };

    public PeerLink(String name, LocalServerSocket server, IntPredicate admitsUid, String bundle, Loop loop) {
        this.name = name;
        this.server = server;
        this.admitsUid = admitsUid;
        this.bundle = bundle;
        this.loop = loop;
    }

    public void start(Peer linkPeer) {
        this.peer = linkPeer;
        new Thread(this::acceptLoop, name + "-accept").start();
    }

    @Override
    public void send(Record record) {
        Connection connection = current;
        if (connection != null && handshaken) {
            connection.send(record);
        }
    }

    private void acceptLoop() {
        while (true) {
            try {
                LocalSocket socket = server.accept();
                Credentials peerCredentials = socket.getPeerCredentials();
                if (!admitsUid.test(peerCredentials.getUid())) {
                    AgentLog.warn(name + ": refused a peer with uid " + peerCredentials.getUid());
                    socket.close();
                    continue;
                }
                loop.execute(() -> adopt(socket));
            } catch (IOException e) {
                AgentLog.warn(name + ": accept failed: " + e.getMessage());
                SystemClock.sleep(ACCEPT_RETRY_MILLIS);
            }
        }
    }

    private void adopt(LocalSocket socket) {
        Connection previous = current;
        handshaken = false;
        Connection connection = new Connection(name, socket, receiver);
        current = connection;
        if (previous != null) {
            AgentLog.info(name + ": a new peer replaces the connected one");
            previous.close();
        }
        connection.start();
        connection.send(new Hello(WireCodec.WIRE_VERSION, bundle));
    }

    private void handle(Connection connection, Record record) {
        if (connection != current) {
            return;
        }
        if (!handshaken) {
            if (record instanceof Hello && ((Hello) record).wire == WireCodec.WIRE_VERSION) {
                handshaken = true;
                AgentLog.info(name + ": peer connected, bundle " + ((Hello) record).bundle);
                peer.onConnected();
            } else {
                AgentLog.warn(name + ": first record is not a wire " + WireCodec.WIRE_VERSION + " Hello");
                connection.close();
            }
            return;
        }
        if (!peer.onRecord(record)) {
            AgentLog.warn(name + String.format(": record type 0x%04x does not belong on this socket", record.type()));
            connection.close();
        }
    }
}
