// Shared TypeScript interfaces — derived from the Rust backend contract

export type TransferStatus =
  | "pending"
  | "in_progress"
  | "paused"
  | "completed"
  | "failed";

export interface Transfer {
  id: string;
  peer_addr: string;
  file_path: string;
  file_size: number;
  bytes_transferred: number;
  checksum: string | null;
  status: TransferStatus;
  created_at: string;
  updated_at: string;
}

// list_peers returns this shape
export interface Peer {
  name: string;
  addr: string;
  last_seen: number; // unix timestamp (seconds)
}

// Config fields from the backend TOML contract
export interface Config {
  tcp_port: string;
  chunk_size: string;
  compression_level: string;
  discovery_port: string;
  data_dir: string;
  relay_server: string;
  directory_identifier: string;
  output_dir: string;
  [key: string]: string;
}

// Contact stored in the DB
export interface ContactEntry {
  name: string;
  node_id: string;
}

// Incoming transfer event from Tauri backend
export interface IncomingTransferEvent {
  request_id: string;
  sender_name: string;
  sender_node_id: string;
  file_name: string;
  file_size: number;
}

// Transfer progress event
export interface TransferProgressEvent {
  request_id: string;
  bytes_transferred: number;
  total: number;
}

// Transfer complete event
export interface TransferCompleteEvent {
  request_id: string;
  file_path: string;
  blake3_hash: string;
  elapsed_secs: number;
}
