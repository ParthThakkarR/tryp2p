/**
 * ChunkFlowBar — Signature element.
 *
 * Visualizes a file transfer as a row of discrete chunk-cells that
 * light up left-to-right as bytes_transferred advances. The leading-
 * edge cell glows, giving a real-time sense of the encrypted stream landing.
 *
 * Props:
 *   fileSize         — total bytes (from Transfer.file_size)
 *   bytesTransferred — current bytes (from Transfer.bytes_transferred)
 *   chunkSizeBytes   — chunk size from config (default 2MB if unknown)
 *   status           — drives color / glow states
 *   thumb            — compact mode for table rows
 */

interface Props {
  fileSize: number;
  bytesTransferred: number;
  chunkSizeBytes?: number;
  status: string;
  thumb?: boolean;
  showMeta?: boolean;
}

const DEFAULT_CHUNK = 2 * 1024 * 1024; // 2MB fallback

function formatBytes(b: number): string {
  if (b === 0) return "0 B";
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / (1024 * 1024)).toFixed(1)} MB`;
  return `${(b / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function ChunkFlowBar({
  fileSize,
  bytesTransferred,
  chunkSizeBytes = DEFAULT_CHUNK,
  status,
  thumb = false,
  showMeta = true,
}: Props) {
  const statusLower = status.toLowerCase();
  const isActive    = statusLower === "in_progress";
  const isFailed    = statusLower === "failed";
  const isCompleted = statusLower === "completed";

  // Clamp cell count between 20 and 80 for visual density
  const rawCells = fileSize > 0 ? Math.ceil(fileSize / chunkSizeBytes) : 30;
  const numCells = Math.max(20, Math.min(80, rawCells));

  const progress = fileSize > 0 ? bytesTransferred / fileSize : 0;
  const filledCount = Math.round(progress * numCells);
  const pct = Math.round(progress * 100);

  return (
    <div className="chunk-flow-container">
      {showMeta && !thumb && (
        <div className="chunk-flow-meta">
          <span className="chunk-flow-pct">{pct}%</span>
          <span className="chunk-flow-bytes">
            {formatBytes(bytesTransferred)} / {formatBytes(fileSize)}
          </span>
        </div>
      )}
      <div className={`chunk-flow-track${thumb ? " thumb" : ""}`}>
        {Array.from({ length: numCells }, (_, i) => {
          const isFilled  = i < filledCount;
          const isLeading = isActive && i === filledCount - 1;

          let cellClass = "chunk-cell";
          if (isFilled) {
            cellClass += " filled";
            if (isLeading) cellClass += " leading";
            else if (isCompleted) cellClass += " completed";
            else if (isFailed) cellClass += " failed";
          }

          return <div key={i} className={cellClass} />;
        })}
      </div>
    </div>
  );
}

// Re-export the formatter for reuse
export { formatBytes };
