use anyhow::{Context, Result};
use std::path::Path;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::io::{AsyncBufReadExt, BufReader};
use std::time::Duration;

pub struct WanTunnel {
    pub url: String,
    copyparty: Child,
    cloudflared: Child,
    pub token: String,
}

impl WanTunnel {
    pub async fn start(
        copyparty_bin: &Path,
        cloudflared_bin: &Path,
        share_path: &Path,
        token: String,
        tunnel_token: Option<String>,
    ) -> Result<Self> {
        let port = get_available_port().await.context("Failed to get available port")?;
        
        let mut copyparty = start_copyparty(copyparty_bin, share_path, port)?;
        let mut cloudflared = start_cloudflared(cloudflared_bin, port, tunnel_token.clone())?;
        
        let stderr = cloudflared.stderr.take().context("No stderr for cloudflared")?;
        let mut reader = BufReader::new(stderr).lines();
        let mut url = None;
        
        let timeout = tokio::time::sleep(Duration::from_secs(30));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                line = reader.next_line() => {
                    match line {
                        Ok(Some(l)) => {
                            if l.contains("https://") && l.contains(".trycloudflare.com") {
                                let start = l.find("https://").unwrap();
                                let end = l[start..].find(|c: char| c.is_whitespace() || c == '|').unwrap_or(l.len() - start);
                                url = Some(l[start..start+end].to_string());
                                break;
                            }
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
                _ = &mut timeout => {
                    let _ = copyparty.kill().await;
                    let _ = cloudflared.kill().await;
                    anyhow::bail!("Timeout waiting for cloudflared URL");
                }
            }
        }
        
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open("d:\\tryp2p\\cloudflared_debug.log").await.unwrap();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = file.write_all(format!("{}\n", line).as_bytes()).await;
            }
        });

        let url = url.context("Failed to find trycloudflare URL in cloudflared output")?;

        Ok(Self {
            url,
            copyparty,
            cloudflared,
            token,
        })
    }

    pub async fn stop(&mut self) {
        let _ = self.copyparty.kill().await;
        let _ = self.cloudflared.kill().await;
    }
}

impl Drop for WanTunnel {
    fn drop(&mut self) {
        // Fallback sync kill if stop wasn't explicitly called
        let _ = self.copyparty.start_kill();
        let _ = self.cloudflared.start_kill();
    }
}

fn start_copyparty(bin: &Path, share_path: &Path, port: u16) -> Result<Child> {
    let mut cmd = if bin.extension().map(|s| s == "py").unwrap_or(false) {
        let mut c = Command::new("python3");
        c.arg(bin);
        c
    } else {
        Command::new(bin)
    };
    
    // Serve the share_path read-only
    cmd.current_dir(share_path)
       .arg("-p").arg(port.to_string())
       .arg("-i").arg("127.0.0.1")
       .arg("-q")
       .stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());
       
    let mut child = cmd.spawn().context("Failed to spawn copyparty")?;
    
    if let Some(stdout) = child.stdout.take() {
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stdout);
            let mut line = String::new();
            while let Ok(n) = tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await {
                if n == 0 { break; }
                println!("copyparty stdout: {}", line.trim());
                line.clear();
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await {
                if n == 0 { break; }
                println!("copyparty stderr: {}", line.trim());
                line.clear();
            }
        });
    }
    println!("copyparty bound to port {}", port);
    Ok(child)
}

fn start_cloudflared(bin: &Path, port: u16, tunnel_token: Option<String>) -> Result<Child> {
    if let Some(token) = tunnel_token {
        Command::new(bin)
            .arg("tunnel")
            .arg("--url")
            .arg(format!("http://127.0.0.1:{}", port))
            .arg("run")
            .arg("--token")
            .arg(token)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn authenticated cloudflared")
    } else {
        println!("⚠️ WARNING: Using Cloudflare Quick Tunnels. Speeds may be severely throttled (e.g., <1 MiB/s).");
        println!("⚠️ Add a tunnel_token in config to use an authenticated tunnel for better speeds.");
        Command::new(bin)
            .arg("tunnel")
            .arg("--url")
            .arg(format!("http://127.0.0.1:{}", port))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn cloudflared")
    }
}

async fn get_available_port() -> Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    Ok(port)
}
