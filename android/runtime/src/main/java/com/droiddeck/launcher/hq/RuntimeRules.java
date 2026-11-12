package com.droiddeck.launcher.hq;

import java.io.File;
import java.io.IOException;

/** Pure validation shared by the Android bridge and its JVM checks. */
public final class RuntimeRules {
    private RuntimeRules() {}

    public static boolean hqGameControls(boolean installing, String requestedSteamUrl, String activeSteamUrl) {
        // SessionActivity builds its controls before SessionService fills SessionState on a cold start.
        String url = activeSteamUrl != null ? activeSteamUrl : requestedSteamUrl;
        return !installing && "steam://rungameid/1966720".equals(url);
    }

    public static boolean sessionStopped(boolean running, String phase, boolean stopRequested, boolean runtimeBusy, boolean launchPending) {
        return !running && !stopRequested && !runtimeBusy && !launchPending &&
            ("idle".equals(phase) || "failed".equals(phase));
    }

    public static String withWinHttp(String value) {
        java.util.List<String> entries = new java.util.ArrayList<>();
        if (value != null) for (String entry : value.split(";")) {
            String[] pair = entry.trim().split("=", 2);
            if (pair.length != 2) continue;
            java.util.List<String> names = new java.util.ArrayList<>();
            for (String name : pair[0].split(",")) {
                name = name.trim();
                if (!name.isEmpty() && !name.equalsIgnoreCase("winhttp")) names.add(name);
            }
            if (!names.isEmpty()) entries.add(String.join(",", names) + "=" + pair[1]);
        }
        entries.add("winhttp=n,b");
        return String.join(";", entries);
    }

    public static File internalDirectory(File privateRoot, File runtimeRoot, File candidate) throws IOException {
        File root = privateRoot.getCanonicalFile();
        File linux = runtimeRoot.getCanonicalFile();
        File game = candidate.getCanonicalFile();
        if (!linux.toPath().startsWith(root.toPath()) || linux.equals(root) ||
            !game.toPath().startsWith(linux.toPath()) || game.equals(linux) || !game.isDirectory()) {
            throw new IOException("Only HQ internal game directories are supported.");
        }
        return game;
    }
}
