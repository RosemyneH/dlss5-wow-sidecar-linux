use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuArch {
    Ada,
    Blackwell,
    Other,
    Unsupported,
}

impl GpuArch {
    pub fn as_str(self) -> &'static str {
        match self {
            GpuArch::Blackwell => "Blackwell (RTX 50)",
            GpuArch::Ada => "Ada (RTX 40)",
            GpuArch::Other => "Other NVIDIA",
            GpuArch::Unsupported => "Unsupported",
        }
    }

    pub fn sidecar_supported(self) -> bool {
        matches!(self, GpuArch::Ada | GpuArch::Blackwell)
    }
}

#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub name: String,
    pub driver_version: Option<String>,
    pub arch: GpuArch,
}

pub fn architecture_from_name(name: &str) -> GpuArch {
    let n = name.to_ascii_lowercase();
    if n.contains("rtx 50") || n.contains("rtx 5090") || n.contains("rtx 5080") {
        return GpuArch::Blackwell;
    }
    if n.contains("rtx 40") || n.contains("rtx 4090") || n.contains("rtx 4080") {
        return GpuArch::Ada;
    }
    if n.contains("geforce") || n.contains("nvidia") || n.contains("rtx") {
        return GpuArch::Other;
    }
    GpuArch::Unsupported
}

#[cfg(feature = "nvml")]
mod nvml_backend {
    pub fn detect() -> Option<super::GpuInfo> {
        None
    }
}

#[cfg(not(feature = "nvml"))]
mod nvml_backend {
    pub fn detect() -> Option<super::GpuInfo> {
        None
    }
}

fn detect_via_nvidia_smi() -> Option<GpuInfo> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,driver_version",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return None,
    };
    let line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string();
    if line.is_empty() {
        return None;
    }
    let (name, driver) = parse_csv_pair(&line)?;
    let arch = architecture_from_name(&name);
    Some(GpuInfo {
        name,
        driver_version: Some(driver),
        arch,
    })
}

fn parse_csv_pair(line: &str) -> Option<(String, String)> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }
    parts.push(cur.trim().to_string());
    if parts.len() < 2 {
        return None;
    }
    Some((parts[0].clone(), parts[1].clone()))
}

pub fn detect_primary_gpu() -> Option<GpuInfo> {
    nvml_backend::detect().or_else(detect_via_nvidia_smi)
}
