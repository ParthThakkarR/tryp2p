package com.p2p.app;

import javax.imageio.ImageIO;
import java.awt.*;
import java.awt.image.BufferedImage;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.util.List;

public final class IconGenerator {

    private IconGenerator() {}

    public static void main(String[] args) throws IOException {
        if (args.length < 1) {
            System.err.println("Usage: IconGenerator <output.ico>");
            System.exit(1);
        }
        generate(args[0]);
    }

    public static void generate(String path) throws IOException {
        List<Integer> sizes = List.of(16, 24, 32, 48, 64);
        List<byte[]> pngs = sizes.stream().map(size -> {
            BufferedImage img = new BufferedImage(size, size, BufferedImage.TYPE_INT_ARGB);
            Graphics2D g = img.createGraphics();
            try {
                g.setRenderingHint(RenderingHints.KEY_ANTIALIASING, RenderingHints.VALUE_ANTIALIAS_ON);
                g.setColor(new Color(41, 128, 185));
                g.fillOval(1, 1, size - 2, size - 2);
                g.setColor(Color.WHITE);
                Font font = new Font("SansSerif", Font.BOLD, size / 3);
                g.setFont(font);
                FontMetrics fm = g.getFontMetrics();
                String text = size < 32 ? "P2" : "P2P";
                int x = (size - fm.stringWidth(text)) / 2;
                int y = (size - fm.getHeight()) / 2 + fm.getAscent();
                g.drawString(text, x, y);
            } finally {
                g.dispose();
            }
            ByteArrayOutputStream baos = new ByteArrayOutputStream();
            try {
                ImageIO.write(img, "PNG", baos);
            } catch (IOException e) {
                throw new RuntimeException(e);
            }
            return baos.toByteArray();
        }).toList();

        int count = pngs.size();
        int headerSize = 6;
        int dirEntrySize = 16;
        int offset = headerSize + count * dirEntrySize;
        ByteArrayOutputStream buf = new ByteArrayOutputStream();

        // ICO header
        buf.write(0); buf.write(0);
        buf.write(1); buf.write(0);
        buf.write(count & 0xFF); buf.write((count >> 8) & 0xFF);

        for (int i = 0; i < sizes.size(); i++) {
            byte[] png = pngs.get(i);
            int s = sizes.get(i);
            buf.write(s >= 256 ? 0 : s);
            buf.write(s >= 256 ? 0 : s);
            buf.write(0);
            buf.write(0);
            buf.write(1); buf.write(0);
            buf.write(32); buf.write(0);
            int sz = png.length;
            buf.write(sz & 0xFF);
            buf.write((sz >> 8) & 0xFF);
            buf.write((sz >> 16) & 0xFF);
            buf.write((sz >> 24) & 0xFF);
            buf.write(offset & 0xFF);
            buf.write((offset >> 8) & 0xFF);
            buf.write((offset >> 16) & 0xFF);
            buf.write((offset >> 24) & 0xFF);
            offset += sz;
        }

        for (byte[] png : pngs) {
            buf.write(png);
        }

        java.nio.file.Path out = java.nio.file.Paths.get(path);
        java.nio.file.Files.createDirectories(out.getParent());
        java.nio.file.Files.write(out, buf.toByteArray());
    }
}
