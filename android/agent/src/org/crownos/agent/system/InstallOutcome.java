package org.crownos.agent.system;

/** How an InstallPackage ended, as the InstallResult record reports it. */
public final class InstallOutcome {
    public final boolean ok;
    public final String message;

    private InstallOutcome(boolean ok, String message) {
        this.ok = ok;
        this.message = message;
    }

    static InstallOutcome installed() {
        return new InstallOutcome(true, "");
    }

    static InstallOutcome failed(String message) {
        return new InstallOutcome(false, message);
    }
}
