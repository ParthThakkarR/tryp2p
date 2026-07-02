package com.p2p.app;

import com.p2p.cli.P2PCli;
import com.p2p.core.model.PeerInfo;
import com.p2p.core.model.PeerId;
import com.p2p.core.util.FileUtils;
import com.p2p.core.util.PlatformUtils;

import javax.swing.*;
import javax.swing.border.EmptyBorder;
import javax.swing.plaf.basic.BasicButtonUI;
import java.awt.*;
import java.awt.event.*;
import java.io.File;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;
import java.util.ArrayList;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * Cross-platform Swing GUI for the P2P File Transfer System.
 * <p>
 * Provides a dark-themed graphical interface with panels for discovered peers
 * and queued files, a send button, and a progress bar. Runs alongside the CLI
 * services (discovery, connection manager) via static fields on {@link P2PCli}.
 * Not thread-safe — all UI mutations are performed on the EDT via
 * {@link SwingUtilities#invokeLater}.
 */
public final class P2PGui {

    // --- Constants ---

    private static final Color BG_DARK = new Color(0x1E, 0x1E, 0x2E);
    private static final Color BG_PANEL = new Color(0x28, 0x28, 0x3C);
    private static final Color FG_TEXT = new Color(0xCD, 0xD6, 0xF4);
    private static final Color FG_ACCENT = new Color(0x89, 0xB4, 0xFA);
    private static final Color FG_GREEN = new Color(0xA6, 0xE3, 0xA1);
    private static final Color FG_RED = new Color(0xF3, 0x8B, 0xA8);
    private static final Color FG_ORANGE = new Color(0xFA, 0xB3, 0x87);
    private static final Color BORDER = new Color(0x45, 0x45, 0x5A);

    // --- Fields ---

    private final JFrame frame;
    private final DefaultListModel<PeerInfo> peerListModel = new DefaultListModel<>();
    private final DefaultListModel<FileEntry> fileListModel = new DefaultListModel<>();
    private final JLabel statusLabel = new JLabel();
    private final JProgressBar progressBar = new JProgressBar();
    private final JLabel progressLabel = new JLabel();
    private final JButton sendButton = new JButton("Send");
    private final JLabel saveDirLabel = new JLabel();
    private PeerInfo selectedPeer;
    private Timer refreshTimer;

    // --- Constructor ---

    public P2PGui() {
        frame = new JFrame("P2P File Transfer");
        buildUI();
        startRefreshTimer();
        frame.setVisible(true);
    }

    // --- UI Construction ---

    private void buildUI() {
        frame.setDefaultCloseOperation(JFrame.DO_NOTHING_ON_CLOSE);
        frame.addWindowListener(new WindowAdapter() {
            @Override
            public void windowClosing(WindowEvent e) {
                if (refreshTimer != null) refreshTimer.stop();
                P2PCli.shutdown();
                System.exit(0);
            }
        });
        frame.setSize(900, 620);
        frame.setLocationRelativeTo(null);

        var content = new JPanel(new BorderLayout(0, 0));
        content.setBackground(BG_DARK);
        frame.setContentPane(content);

        content.add(buildTopBar(), BorderLayout.NORTH);

        var centerPanel = new JPanel(new BorderLayout(0, 0));
        centerPanel.setBackground(BG_DARK);
        centerPanel.setBorder(new EmptyBorder(8, 10, 6, 10));

        var split = new JSplitPane(JSplitPane.HORIZONTAL_SPLIT, buildPeerPanel(), buildFilePanel());
        split.setResizeWeight(0.4);
        split.setBorder(null);
        split.setBackground(BG_DARK);
        centerPanel.add(split, BorderLayout.CENTER);

        content.add(centerPanel, BorderLayout.CENTER);

        content.add(buildBottomBar(), BorderLayout.SOUTH);
    }

    private JPanel buildTopBar() {
        var bar = new JPanel(new BorderLayout(10, 0));
        bar.setBackground(BG_PANEL);
        bar.setBorder(new EmptyBorder(14, 16, 12, 16));

        var title = new JLabel("P2P File Transfer");
        title.setFont(title.getFont().deriveFont(Font.BOLD, 16f));
        title.setForeground(FG_ACCENT);
        bar.add(title, BorderLayout.WEST);

        statusLabel.setForeground(FG_TEXT);
        bar.add(statusLabel, BorderLayout.CENTER);

        return bar;
    }

    // --- Peer panel ---

    private JPanel buildPeerPanel() {
        var panel = new JPanel(new BorderLayout(0, 6));
        panel.setBackground(BG_DARK);
        panel.setBorder(new EmptyBorder(0, 0, 0, 4));

        var header = new JLabel("Discovered Peers");
        header.setFont(header.getFont().deriveFont(Font.BOLD, 13f));
        header.setForeground(FG_ACCENT);
        panel.add(header, BorderLayout.NORTH);

        var list = new JList<>(peerListModel);
        list.setCellRenderer(new PeerCellRenderer());
        list.setBackground(BG_PANEL);
        list.setForeground(FG_TEXT);
        list.setSelectionMode(ListSelectionModel.SINGLE_SELECTION);
        list.addListSelectionListener(e -> {
            if (!e.getValueIsAdjusting()) {
                selectedPeer = list.getSelectedValue();
                updateSendButton();
            }
        });
        list.addMouseListener(new MouseAdapter() {
            @Override
            public void mouseClicked(MouseEvent e) {
                if (e.getClickCount() == 2 && selectedPeer != null && !fileListModel.isEmpty()) {
                    doSend();
                }
            }
        });
        panel.add(new JScrollPane(list), BorderLayout.CENTER);

        var refreshBtn = new JButton("\u21BB Refresh");
        styleButton(refreshBtn);
        refreshBtn.addActionListener(e -> refreshPeers());
        panel.add(refreshBtn, BorderLayout.SOUTH);

        return panel;
    }

    // --- File panel ---

    private JPanel buildFilePanel() {
        var panel = new JPanel(new BorderLayout(0, 6));
        panel.setBackground(BG_DARK);
        panel.setBorder(new EmptyBorder(0, 4, 0, 0));

        var header = new JLabel("Files to Send");
        header.setFont(header.getFont().deriveFont(Font.BOLD, 13f));
        header.setForeground(FG_ACCENT);
        panel.add(header, BorderLayout.NORTH);

        var list = new JList<>(fileListModel);
        list.setCellRenderer(new FileCellRenderer());
        list.setBackground(BG_PANEL);
        list.setForeground(FG_TEXT);
        list.addMouseListener(new MouseAdapter() {
            @Override
            public void mouseClicked(MouseEvent e) {
                if (e.getClickCount() == 2) {
                    int idx = list.locationToIndex(e.getPoint());
                    if (idx >= 0) fileListModel.remove(idx);
                }
            }
        });
        panel.add(new JScrollPane(list), BorderLayout.CENTER);

        var btnRow = new JPanel(new FlowLayout(FlowLayout.LEFT, 6, 0));
        btnRow.setBackground(BG_DARK);

        var addBtn = new JButton("+ Add Files");
        styleButton(addBtn);
        addBtn.setForeground(FG_GREEN);
        addBtn.addActionListener(e -> addFiles());
        btnRow.add(addBtn);

        var addDirBtn = new JButton("+ Add Folder");
        styleButton(addDirBtn);
        addDirBtn.setForeground(FG_ORANGE);
        addDirBtn.addActionListener(e -> addFolder());
        btnRow.add(addDirBtn);

        var clearBtn = new JButton("Clear All");
        styleButton(clearBtn);
        clearBtn.setForeground(FG_RED);
        clearBtn.addActionListener(e -> fileListModel.clear());
        btnRow.add(clearBtn);

        panel.add(btnRow, BorderLayout.SOUTH);

        return panel;
    }

    // --- Bottom bar ---

    private JPanel buildBottomBar() {
        var bar = new JPanel(new BorderLayout(8, 0));
        bar.setBackground(BG_PANEL);
        bar.setBorder(new EmptyBorder(10, 14, 10, 14));

        var left = new JPanel(new FlowLayout(FlowLayout.LEFT, 6, 0));
        left.setBackground(BG_PANEL);
        left.setBorder(new EmptyBorder(0, 4, 0, 0));

        progressBar.setPreferredSize(new Dimension(200, 18));
        progressBar.setStringPainted(true);
        progressBar.setForeground(FG_ACCENT);
        progressBar.setBackground(BG_DARK);
        progressBar.setBorder(BorderFactory.createLineBorder(BORDER));
        progressBar.setVisible(false);
        left.add(progressBar);

        progressLabel.setForeground(FG_TEXT);
        progressLabel.setVisible(false);
        left.add(progressLabel);

        bar.add(left, BorderLayout.CENTER);

        var right = new JPanel(new FlowLayout(FlowLayout.RIGHT, 8, 0));
        right.setBackground(BG_PANEL);

        saveDirLabel.setForeground(FG_TEXT);
        saveDirLabel.setFont(saveDirLabel.getFont().deriveFont(11f));
        updateSaveDirLabel();
        right.add(saveDirLabel);

        var changeDirBtn = new JButton("...");
        styleButton(changeDirBtn);
        changeDirBtn.setForeground(FG_ACCENT);
        changeDirBtn.setToolTipText("Change save directory");
        changeDirBtn.addActionListener(e -> chooseSaveDir());
        right.add(changeDirBtn);

        sendButton.setForeground(Color.BLACK);
        sendButton.setBackground(FG_ACCENT);
        sendButton.setFont(sendButton.getFont().deriveFont(Font.BOLD));
        sendButton.setFocusPainted(false);
        sendButton.setBorderPainted(true);
        sendButton.addActionListener(e -> doSend());
        right.add(sendButton);
        bar.add(right, BorderLayout.EAST);

        return bar;
    }

    private static void styleButton(JButton btn) {
        btn.setBackground(BG_PANEL);
        btn.setForeground(FG_TEXT);
        btn.setFocusPainted(false);
        btn.setBorderPainted(false);
        btn.setCursor(Cursor.getPredefinedCursor(Cursor.HAND_CURSOR));
        btn.setFont(btn.getFont().deriveFont(12f));
    }

    // --- Timers & updates ---

    private void startRefreshTimer() {
        refreshTimer = new Timer(3000, e -> {
            refreshPeers();
            updateStatusBar();
        });
        refreshTimer.start();
    }

    private void refreshPeers() {
        if (P2PCli.discovery == null) return;
        var current = P2PCli.discovery.getDiscoveredPeers();
        peerListModel.clear();
        for (var p : current) {
            peerListModel.addElement(p);
        }
        updateSendButton();
    }

    private void updateStatusBar() {
        String id = P2PCli.localPeerId != null ? P2PCli.localPeerId.toShortString() : "...";
        int peers = peerListModel.getSize();
        int conns = P2PCli.connectionManager != null ? P2PCli.connectionManager.getActiveCount() : 0;
        int xfers = P2PCli.activeTransfers != null ? P2PCli.activeTransfers.size() : 0;
        statusLabel.setText(String.format("Peer: %s  |  Discovered: %d  |  Connections: %d  |  Transfers: %d",
                id, peers, conns, xfers));
    }

    private void updateSaveDirLabel() {
        String p = P2PCli.receiveSaveDir.toAbsolutePath().toString();
        saveDirLabel.setText("Save: " + p);
    }

    private void chooseSaveDir() {
        var chooser = new JFileChooser(P2PCli.receiveSaveDir.toFile());
        chooser.setFileSelectionMode(JFileChooser.DIRECTORIES_ONLY);
        chooser.setDialogTitle("Choose Save Directory");
        if (chooser.showOpenDialog(frame) == JFileChooser.APPROVE_OPTION) {
            P2PCli.receiveSaveDir = chooser.getSelectedFile().toPath();
            updateSaveDirLabel();
        }
    }

    private void updateSendButton() {
        sendButton.setEnabled(selectedPeer != null && !fileListModel.isEmpty());
        if (selectedPeer != null) {
            sendButton.setText("Send to " + selectedPeer.getDisplayName());
        } else {
            sendButton.setText("Send");
        }
    }

    // --- File operations ---

    /**
     * Opens a file chooser dialog for selecting multiple files and adds them to
     * the file list model.
     */
    private void addFiles() {
        var chooser = new JFileChooser();
        chooser.setMultiSelectionEnabled(true);
        chooser.setDialogTitle("Select Files");
        if (chooser.showOpenDialog(frame) == JFileChooser.APPROVE_OPTION) {
            for (var f : chooser.getSelectedFiles()) {
                addFile(f.toPath());
            }
        }
    }

    /**
     * Opens a directory chooser dialog and adds the selected folder to the file
     * list model.
     */
    private void addFolder() {
        var chooser = new JFileChooser();
        chooser.setFileSelectionMode(JFileChooser.DIRECTORIES_ONLY);
        chooser.setDialogTitle("Select Folder");
        if (chooser.showOpenDialog(frame) == JFileChooser.APPROVE_OPTION) {
            addFile(chooser.getSelectedFile().toPath());
        }
    }

    private void addFile(Path path) {
        File f = path.toFile();
        String name = f.getName();
        long size = f.length();
        if (f.isDirectory()) {
            try { size = FileUtils.totalSize(path); } catch (Exception ignored) { size = 0; }
        }
        fileListModel.addElement(new FileEntry(path, name, size, f.isDirectory()));
    }

    // --- Transfer ---

    /**
     * Initiates sending all queued files to the selected peer.
     * <p>
     * Disables the send button, shows the progress bar, and spawns a background
     * thread that invokes {@code p2p send} via picocli for each file. Updates
     * progress on the EDT after each completed file.
     */
    private void doSend() {
        if (selectedPeer == null || fileListModel.isEmpty()) return;

        var files = new ArrayList<FileEntry>();
        for (int i = 0; i < fileListModel.size(); i++) {
            files.add(fileListModel.get(i));
        }

        sendButton.setEnabled(false);
        progressBar.setValue(0);
        progressBar.setVisible(true);
        progressLabel.setVisible(true);

        String peerName = selectedPeer.getDisplayName();
        progressLabel.setText("Starting...");

        new Thread(() -> {
            int total = files.size();
            var completed = new AtomicInteger(0);
            for (var entry : files) {
                try {
                    String path = entry.path().toString();
                    int exitCode = new picocli.CommandLine(new P2PCli())
                            .execute("send", path, peerName);
                    SwingUtilities.invokeLater(() -> {
                        int done = completed.incrementAndGet();
                        int pct = (int) ((done / (double) total) * 100);
                        progressBar.setValue(pct);
                        progressLabel.setText(entry.name() + " done  (" + done + "/" + total + ")");
                    });
                } catch (Exception ex) {
                    SwingUtilities.invokeLater(() -> {
                        progressLabel.setText("Failed: " + entry.name() + " - " + ex.getMessage());
                    });
                }
            }
            SwingUtilities.invokeLater(() -> {
                progressLabel.setText("All transfers complete");
                sendButton.setEnabled(true);
            });
        }).start();
    }

    // --- Data classes ---

    record FileEntry(Path path, String name, long size, boolean isDirectory) {
        @Override
        public String toString() {
            String sizeStr = isDirectory ? "<DIR>" : formatSize(size);
            return name + "  (" + sizeStr + ")";
        }
    }

    static String formatSize(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return String.format("%.1f KB", bytes / 1024.0);
        if (bytes < 1024L * 1024 * 1024) return String.format("%.1f MB", bytes / (1024.0 * 1024));
        return String.format("%.2f GB", bytes / (1024.0 * 1024 * 1024));
    }

    // --- Renderers ---

    private static class PeerCellRenderer extends JLabel implements ListCellRenderer<PeerInfo> {
        PeerCellRenderer() {
            setOpaque(true);
            setBorder(new EmptyBorder(4, 8, 4, 8));
        }

        @Override
        public Component getListCellRendererComponent(JList<? extends PeerInfo> list, PeerInfo p,
                                                       int index, boolean selected, boolean focused) {
            setText(String.format("%-16s  %s:%d", p.getDisplayName(),
                    p.getAddress().getHostAddress(), p.getPort()));
            if (selected) {
                setBackground(BG_PANEL.brighter());
            } else {
                setBackground(BG_PANEL);
            }
            setForeground(p.getStatus() != null && p.getStatus().name().contains("ONLINE") ? FG_GREEN : FG_TEXT);
            return this;
        }
    }

    private static class FileCellRenderer extends JLabel implements ListCellRenderer<FileEntry> {
        FileCellRenderer() {
            setOpaque(true);
            setBorder(new EmptyBorder(4, 8, 4, 8));
        }

        @Override
        public Component getListCellRendererComponent(JList<? extends FileEntry> list, FileEntry f,
                                                       int index, boolean selected, boolean focused) {
            setText(f.toString());
            if (selected) {
                setBackground(BG_PANEL.brighter());
            } else {
                setBackground(BG_PANEL);
            }
            setForeground(f.isDirectory() ? FG_ORANGE : FG_TEXT);
            return this;
        }
    }

    // --- Platform setup ---

    /**
     * Configures platform-specific Swing properties and applies the system
     * look-and-feel with a dark theme override.
     * <p>
     * On macOS: enables screen menu bar and native window chrome.
     * On Linux: enables OpenGL rendering acceleration.
     * On all platforms: applies the system L&amp;F, then overrides key
     * UIManager defaults for the dark theme.
     */
    private static void setupPlatform() {
        String os = System.getProperty("os.name").toLowerCase();

        if (os.contains("mac")) {
            // macOS: integrate with screen menu bar and set app name
            System.setProperty("apple.awt.application.name", "P2PTransfer");
            System.setProperty("apple.awt.application.appearance", "system");
            try {
                // Use native menu bar instead of in-window
                System.setProperty("apple.laf.useScreenMenuBar", "true");
            } catch (Exception ignored) {}
        } else if (os.contains("linux") || os.contains("nix") || os.contains("nux")) {
            // Linux: set some desktop hints
            System.setProperty("sun.java2d.opengl", "true");
        }

        // Apply system L&F for native dialogs/widgets, then override colors
        try {
            UIManager.setLookAndFeel(UIManager.getSystemLookAndFeelClassName());
        } catch (Exception ignored) {}

        // Apply dark theme to UIManager for consistent look
        UIManager.put("Panel.background", BG_DARK);
        UIManager.put("OptionPane.background", BG_DARK);
        UIManager.put("OptionPane.messageForeground", FG_TEXT);
        UIManager.put("TextField.background", BG_PANEL);
        UIManager.put("TextField.foreground", FG_TEXT);
        UIManager.put("TextField.caretForeground", FG_TEXT);
        UIManager.put("List.background", BG_PANEL);
        UIManager.put("List.foreground", FG_TEXT);
        UIManager.put("ScrollPane.background", BG_DARK);
        UIManager.put("Viewport.background", BG_PANEL);
        UIManager.put("ProgressBar.background", BG_DARK);
        UIManager.put("ProgressBar.foreground", FG_ACCENT);
        UIManager.put("ProgressBar.selectionForeground", FG_TEXT);
        UIManager.put("ProgressBar.selectionBackground", FG_TEXT);
    }

    // --- Entry point ---

    /**
     * Launches the GUI application.
     * <p>
     * Performs platform setup, initializes P2P services, and creates the
     * main window on the EDT.
     */
    public static void launch() {
        // Cross-platform setup
        setupPlatform();

        P2PCli.initialize();
        SwingUtilities.invokeLater(P2PGui::new);
    }

    public static void main(String[] args) {
        launch();
    }
}
