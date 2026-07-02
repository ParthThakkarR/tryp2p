package com.p2p.app;

import com.p2p.cli.P2PCli;
import java.util.Arrays;

/**
 * Application entry point for the P2P File Transfer System.
 * <p>
 * Routes execution to either the Swing GUI ({@code --gui} flag) or the
 * picocli-based CLI (all other invocations). Thread-safe by design;
 * delegation is single-threaded at startup.
 */
public final class P2PApplication {

    /** Current application version string. */
    public static final String VERSION = "1.0.0-SNAPSHOT";

    /**
     * Application entry point.
     * <p>
     * If the sole argument is {@code --gui}, launches the Swing GUI via
     * {@link P2PGui#launch()}. Otherwise prints a version banner and
     * delegates to {@link P2PCli#main(String[])}.
     *
     * @param args command-line arguments
     */
    public static void main(String[] args) {
        if (args.length >= 1 && (args[0].equals("--cli") || args[0].equals("--interactive"))) {
            String[] rest = Arrays.copyOfRange(args, 1, args.length);
            System.out.println("P2P File Transfer System v" + VERSION);
            P2PCli.main(rest);
        } else {
            P2PGui.launch();
        }
    }
}
