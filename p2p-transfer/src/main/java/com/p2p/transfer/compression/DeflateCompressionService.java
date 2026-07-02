package com.p2p.transfer.compression;

import com.p2p.core.service.CompressionService;

import java.io.ByteArrayOutputStream;
import java.util.Arrays;
import java.util.Objects;
import java.util.zip.DataFormatException;
import java.util.zip.Deflater;
import java.util.zip.Inflater;

/**
 * Deflate (LZ77 + Huffman coding) compression service with adaptive ratio
 * estimation. Provides standard compress/decompress operations as well as
 * a sampling-based compression ratio estimator used to decide whether
 * compression is beneficial. Thread-safe (stateless).
 */
public final class DeflateCompressionService implements CompressionService {

    // --- Constants ---

    private static final int BUFFER_SIZE = 8192;
    private static final int SAMPLE_SIZE = 65536; // 64 KB sample for estimation

    // --- Public API ---

    /**
     * Compresses the given data using Deflate at the default compression
     * level.
     *
     * @param data the plaintext to compress
     * @return the compressed bytes; an empty array if input is null or empty
     * @throws NullPointerException if data is null
     */
    @Override
    public byte[] compress(byte[] data) {
        Objects.requireNonNull(data, "data must not be null");
        if (data.length == 0) {
            return new byte[0];
        }

        Deflater deflater = new Deflater(Deflater.DEFAULT_COMPRESSION);
        deflater.setInput(data);
        deflater.finish();

        ByteArrayOutputStream baos = new ByteArrayOutputStream(data.length);
        byte[] buffer = new byte[BUFFER_SIZE];

        while (!deflater.finished()) {
            int count = deflater.deflate(buffer);
            baos.write(buffer, 0, count);
        }
        deflater.end();
        return baos.toByteArray();
    }

    /**
     * Decompresses data that was previously compressed with Deflate.
     *
     * @param compressedData the compressed bytes
     * @return the decompressed plaintext; an empty array if input is null or empty
     * @throws NullPointerException if compressedData is null
     * @throws RuntimeException     if decompression fails due to corrupt data
     */
    @Override
    public byte[] decompress(byte[] compressedData) {
        Objects.requireNonNull(compressedData, "compressedData must not be null");
        if (compressedData.length == 0) {
            return new byte[0];
        }

        try {
            Inflater inflater = new Inflater();
            inflater.setInput(compressedData);

            ByteArrayOutputStream baos = new ByteArrayOutputStream(compressedData.length * 2);
            byte[] buffer = new byte[BUFFER_SIZE];

            while (!inflater.finished()) {
                int count = inflater.inflate(buffer);
                if (count == 0 && inflater.needsInput()) {
                    break;
                }
                baos.write(buffer, 0, count);
            }
            inflater.end();
            return baos.toByteArray();
        } catch (DataFormatException e) {
            throw new RuntimeException("Decompression failed: corrupt data", e);
        }
    }

    /**
     * Estimates the compression ratio for a sample of data. Samples larger
     * than 64 KB are truncated before compression.
     *
     * @param sample the data sample to test
     * @return the ratio of compressed size to original size (1.0 if sample is
     *         null or empty)
     * @throws NullPointerException if sample is null
     */
    @Override
    public double estimateCompressionRatio(byte[] sample) {
        Objects.requireNonNull(sample, "sample must not be null");
        if (sample.length == 0) {
            return 1.0;
        }

        byte[] toCompress = sample.length > SAMPLE_SIZE
                ? Arrays.copyOf(sample, SAMPLE_SIZE)
                : sample;

        byte[] compressed = compress(toCompress);
        return (double) compressed.length / toCompress.length;
    }

    // --- Object ---

    @Override
    public String toString() {
        return String.format("DeflateCompressionService[bufferSize=%d, sampleSize=%d]",
                BUFFER_SIZE, SAMPLE_SIZE);
    }
}
