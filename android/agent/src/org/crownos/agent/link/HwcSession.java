package org.crownos.agent.link;

import org.crownos.agent.task.TaskCoordinator;
import org.crownos.agent.wire.HwcCommand;
import org.crownos.agent.wire.Record;

/** The hwcomposer side of {@code /dev/socket/crown_agent}. */
public final class HwcSession implements PeerLink.Peer, HwcCommand.Handler {
    private final TaskCoordinator coordinator;

    public HwcSession(TaskCoordinator coordinator) {
        this.coordinator = coordinator;
    }

    @Override
    public void onConnected() {
        coordinator.resendWindows();
    }

    @Override
    public boolean onRecord(Record record) {
        if (!(record instanceof HwcCommand)) {
            return false;
        }
        ((HwcCommand) record).dispatch(this);
        return true;
    }

    @Override
    public void onSetSize(HwcCommand.SetSize command) {
        coordinator.resize(command.task, command.width, command.height, command.serial);
    }

    @Override
    public void onFocus(HwcCommand.Focus command) {
        coordinator.focus(command.task);
    }

    @Override
    public void onClose(HwcCommand.Close command) {
        coordinator.close(command.task);
    }

    @Override
    public void onBack(HwcCommand.Back command) {
        coordinator.back(command.task);
    }

    @Override
    public void onVisibility(HwcCommand.Visibility command) {
        coordinator.setHostVisible(command.task, command.visible);
    }
}
