import type { TransferStatus } from "../types";

const STATUS_MAP: Record<
  TransferStatus,
  { label: string; className: string }
> = {
  pending:     { label: "Pending",     className: "pill pill-pending" },
  in_progress: { label: "Transferring", className: "pill pill-in_progress" },
  paused:      { label: "Paused",      className: "pill pill-paused" },
  completed:   { label: "Completed",   className: "pill pill-completed" },
  failed:      { label: "Failed",      className: "pill pill-failed" },
};

interface Props {
  status: string; // accept string so we can handle casing variants gracefully
}

export function StatusPill({ status }: Props) {
  const key = status.toLowerCase() as TransferStatus;
  const cfg = STATUS_MAP[key] ?? { label: status, className: "pill pill-pending" };

  return (
    <span className={cfg.className}>
      <span className="pill-dot" />
      {cfg.label}
    </span>
  );
}
