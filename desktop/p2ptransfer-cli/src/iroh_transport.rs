/// iroh-blobs based transport — EXPERIMENTAL, additive alongside existing TCP path.
///
/// Uses iroh 1.0.2 / iroh-blobs 0.103.0 QUIC+NAT-traversal stack.
/// Existing `network/tcp.rs` and `transfer/engine.rs` are NOT modified.
use anyhow::{Context, Result};
use iroh::{endpoint::presets, protocol::Router, Endpoint};
use iroh_blobs::{api::{blobs::AddPathOptions, proto::ImportMode}, store::fs::FsStore, ticket::BlobTicket, BlobFormat, BlobsProtocol};
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

/// Start an iroh node, import `file_path` into an in-process blob store,
/// print the ticket, then serve until the transfer completes.
///
/// Prints:
///   [IROH] Hashing file…
///   [IROH] Share this ticket with the receiver:  <ticket>
///   [IROH] Transfer complete in X.Xs  (connection type: direct|relay)
pub async fn iroh_send_file(file_path: &Path, data_dir: &Path) -> Result<()> {
    println!("[IROH] Starting iroh node…");

    let store_path = data_dir.join("iroh_store_send");
    std::fs::create_dir_all(&store_path)?;

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("Failed to bind iroh endpoint")?;

    let store = FsStore::load(&store_path)
        .await
        .context("Failed to open iroh blob store")?;

    let blobs = BlobsProtocol::new(&store, None);

    let router = Router::builder(endpoint.clone())
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();

    println!("[IROH] Hashing file…");
    let abs_path = std::path::absolute(file_path)
        .context("Failed to resolve absolute path")?;

    let tag = store
        .blobs()
        .add_path_with_opts(AddPathOptions {
            path: abs_path,
            format: BlobFormat::Raw,
            mode: ImportMode::TryReference,
        })
        .await
        .context("Failed to import file into iroh blob store")?;

    let node_id = endpoint.id();
    let ticket = BlobTicket::new(node_id.into(), tag.hash, tag.format);

    println!("[IROH] Share this ticket with the receiver:");
    println!("  {}", ticket);
    println!("[IROH] Waiting for receiver to connect… (Ctrl-C to cancel)");

    let start = Instant::now();

    // Wait for the transfer to complete: monitor the store for the tag to be
    // requested.  iroh-blobs handles the actual serving; we simply keep the
    // router alive until Ctrl-C or completion signal.
    //
    // NOTE: iroh-blobs 0.103 does not expose a "transfer complete" callback on
    // the sender side — it serves blobs passively.  We therefore block until
    // Ctrl-C.  The receiver side prints completion.  This matches the UX of
    // `copyparty` / the WAN path.
    tokio::signal::ctrl_c()
        .await
        .context("Failed to listen for Ctrl-C")?;

    let elapsed = start.elapsed();
    println!("[IROH] Node shutting down (ran for {:.1}s).", elapsed.as_secs_f64());
    router.shutdown().await?;
    Ok(())
}

/// Start an iroh receiver node, listen for an incoming blob described by
/// `ticket_str`, write it to `output_dir`, and print the BLAKE3 hash.
///
/// Returns the ticket string so it can be displayed.
pub async fn iroh_listen(ticket_str: &str, output_dir: &Path, data_dir: &Path) -> Result<String> {
    println!("[IROH] Starting iroh node…");

    let store_path = data_dir.join("iroh_store_recv");
    std::fs::create_dir_all(&store_path)?;
    std::fs::create_dir_all(output_dir)?;

    let ticket = BlobTicket::from_str(ticket_str)
        .context("Failed to parse iroh ticket — is it a valid iroh blob ticket?")?;

    let endpoint = Endpoint::bind(presets::N0)
        .await
        .context("Failed to bind iroh endpoint")?;

    let store = FsStore::load(&store_path)
        .await
        .context("Failed to open iroh blob store")?;

    let blobs = BlobsProtocol::new(&store, None);

    let router = Router::builder(endpoint.clone())
        .accept(iroh_blobs::ALPN, blobs.clone())
        .spawn();

    println!("[IROH] Connecting to sender…");
    let start = Instant::now();

    // Download the blob
    let downloader = store.downloader(&endpoint);
    downloader
        .download(ticket.hash(), Some(ticket.addr().id))
        .await
        .context("iroh blob download failed")?;

    let elapsed = start.elapsed();
    println!("[IROH] Download complete in {:.2}s", elapsed.as_secs_f64());

    // Export blob from store to output_dir
    let out_file = output_dir.join(format!("{}", ticket.hash()));
    store
        .blobs()
        .export(ticket.hash(), out_file.clone())
        .await
        .context("Failed to export blob from iroh store")?;

    // Compute BLAKE3 for manual verification
    let data = std::fs::read(&out_file)?;
    let hash = blake3::hash(&data);
    println!("[IROH] Received file: {}", out_file.display());
    println!("[IROH] BLAKE3 hash:   {}", hash.to_hex());
    println!("[IROH] Transfer time: {:.2}s", elapsed.as_secs_f64());

    router.shutdown().await?;
    Ok(ticket_str.to_string())
}
