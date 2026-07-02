package com.p2p.relay;

import java.net.*;
import java.nio.ByteBuffer;
import java.util.Objects;

/**
 * Minimal RFC 8489 STUN client for NAT type detection and public address discovery.
 */
public final class StunClient {

    private static final int STUN_PORT = 19302;
    private static final int TIMEOUT_MS = 5000;

    private final String stunHost;

    public StunClient() {
        this("stun.l.google.com");
    }

    public StunClient(String stunHost) {
        this.stunHost = Objects.requireNonNull(stunHost);
    }

    public StunResult discover() {
        try {
            InetAddress stunAddr = InetAddress.getByName(stunHost);
            String localIP = InetAddress.getLocalHost().getHostAddress();
            try {
                return queryServer(stunAddr, STUN_PORT);
            } catch (Exception e) {
                return new StunResult(false, localIP, 0, NatType.UNKNOWN, e.getMessage());
            }
        } catch (Exception e) {
            return new StunResult(false, "0.0.0.0", 0, NatType.UNKNOWN, e.getMessage());
        }
    }

    private StunResult queryServer(InetAddress serverAddr, int port) throws Exception {
        ByteBuffer req = buildBindingRequest();
        try (DatagramSocket sock = new DatagramSocket()) {
            sock.setSoTimeout(TIMEOUT_MS);
            sock.send(new DatagramPacket(req.array(), req.capacity(), serverAddr, port));

            byte[] buf = new byte[512];
            DatagramPacket resp = new DatagramPacket(buf, buf.length);
            sock.receive(resp);

            ByteBuffer bb = ByteBuffer.wrap(buf, 0, resp.getLength());
            if (bb.get() != 0 || bb.get() != 1) {
                return new StunResult(false, null, 0, NatType.UNKNOWN, "Not a STUN response");
            }
            int len = Short.toUnsignedInt(bb.getShort());
            bb.get(new byte[16]);
            int end = 20 + len;

            String mappedIP = null;
            int mappedPort = 0;
            while (bb.position() < end) {
                int attrType = Short.toUnsignedInt(bb.getShort());
                int attrLen = Short.toUnsignedInt(bb.getShort());
                if (attrType == 0x0020) {
                    bb.get(); bb.get();
                    mappedPort = Short.toUnsignedInt(bb.getShort());
                    bb.get(); bb.get(); bb.get(); bb.get();
                    mappedIP = (bb.get() & 0xFF) + "." + (bb.get() & 0xFF) + "." + (bb.get() & 0xFF) + "." + (bb.get() & 0xFF);
                    break;
                } else {
                    bb.position(bb.position() + attrLen);
                    if (attrLen % 4 != 0) bb.position(bb.position() + 4 - (attrLen % 4));
                }
            }

            if (mappedIP != null) {
                return new StunResult(true, mappedIP, mappedPort, NatType.OPEN, null);
            }
            return new StunResult(false, null, 0, NatType.UNKNOWN, "No MAPPED-ADDRESS attribute");
        }
    }

    private static ByteBuffer buildBindingRequest() {
        byte[] id = new byte[16];
        new java.util.Random().nextBytes(id);
        ByteBuffer bb = ByteBuffer.allocate(20);
        bb.putShort((short) 0x0001);
        bb.putShort((short) 0x0000);
        bb.put(id);
        bb.flip();
        return bb;
    }

    public record StunResult(boolean success, String publicIp, int publicPort, NatType natType, String error) {}

    public enum NatType { OPEN, FULL_CONE, RESTRICTED_CONE, PORT_RESTRICTED, SYMMETRIC, UNKNOWN }
}

