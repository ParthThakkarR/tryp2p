package com.p2p.core.service;

/**
 * Service interface for data compression and decompression.
 * Implementations should provide lossless compression (e.g., Deflate).
 */
public interface CompressionService {

    /**
     * Compresses data using the underlying algorithm.
     *
     * @param data the uncompressed data
     * @return compressed data
     */
    byte[] compress(byte[] data);

    /**
     * Decompresses data previously compressed by this service.
     *
     * @param compressedData the compressed data
     * @return uncompressed data
     */
    byte[] decompress(byte[] compressedData);

    /**
     * Estimates the compression ratio for a data sample.
     * Returns ratio where &lt; 1.0 means compression is beneficial.
     *
     * @param sample sample data (typically first 64KB of file)
     * @return compression ratio (compressedSize / originalSize)
     */
    double estimateCompressionRatio(byte[] sample);
}
