// wan_constants.rs
//
// Pinned versions and checksums for WAN dependencies (Phase 3).
// Note: If you bump a version here, you MUST update the checksums.

pub const COPYPARTY_VERSION: &str = "v1.20.17";
pub const CLOUDFLARED_VERSION: &str = "2026.7.0";

pub struct BinaryPin {
    pub url: &'static str,
    pub filename: &'static str,
    pub sha256: &'static str,
}

pub const COPYPARTY_WIN: BinaryPin = BinaryPin {
    url: "https://github.com/9001/copyparty/releases/download/v1.20.17/copyparty.exe",
    filename: "copyparty.exe",
    sha256: "fd0144ed82fa6a7b5086ba25ba5e79784f5fe418f21e7eb5c9f02c25fc322707",
};

pub const COPYPARTY_SFX: BinaryPin = BinaryPin {
    url: "https://github.com/9001/copyparty/releases/download/v1.20.17/copyparty-sfx.py",
    filename: "copyparty-sfx.py",
    sha256: "0000000000000000000000000000000000000000000000000000000000000000", // TODO: Replace with actual hash
};

pub const CLOUDFLARED_WIN: BinaryPin = BinaryPin {
    url: "https://github.com/cloudflare/cloudflared/releases/download/2024.1.5/cloudflared-windows-amd64.exe",
    filename: "cloudflared.exe",
    sha256: "0000000000000000000000000000000000000000000000000000000000000000", // TODO: Replace with actual hash
};

pub const CLOUDFLARED_MAC: BinaryPin = BinaryPin {
    url: "https://github.com/cloudflare/cloudflared/releases/download/2024.1.5/cloudflared-darwin-amd64.tgz",
    filename: "cloudflared-darwin-amd64.tgz",
    sha256: "0000000000000000000000000000000000000000000000000000000000000000", // TODO: Replace with actual hash
};

pub const CLOUDFLARED_LINUX: BinaryPin = BinaryPin {
    url: "https://github.com/cloudflare/cloudflared/releases/download/2024.1.5/cloudflared-linux-amd64",
    filename: "cloudflared",
    sha256: "0000000000000000000000000000000000000000000000000000000000000000", // TODO: Replace with actual hash
};

// Provides the appropriate pins for the current host OS
pub fn get_copyparty_pin() -> &'static BinaryPin {
    if cfg!(target_os = "windows") {
        &COPYPARTY_WIN
    } else {
        &COPYPARTY_SFX
    }
}

pub fn get_cloudflared_pin() -> &'static BinaryPin {
    if cfg!(target_os = "windows") {
        &CLOUDFLARED_WIN
    } else if cfg!(target_os = "macos") {
        &CLOUDFLARED_MAC
    } else {
        &CLOUDFLARED_LINUX
    }
}
