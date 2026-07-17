
use tokio::io::AsyncWriteExt;
async fn test<W: tokio::io::AsyncWrite + Unpin>(stream: &mut W) {
    let _ = stream.write_vectored(&[std::io::IoSlice::new(&[1])]).await;
}

