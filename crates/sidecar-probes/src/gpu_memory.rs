use std::process::Command;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GpuMemory {
    pub used_mb: u32,
    pub total_mb: u32,
}

pub fn query_gpu_memory() -> Option<GpuMemory> {
    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=memory.used,memory.total",
            "--format=csv,noheader,nounits",
        ])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return None,
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if line.is_empty() {
        return None;
    }
    let (used, total) = parse_csv_pair(line)?;
    Some(GpuMemory {
        used_mb: parse_mb(&used)?,
        total_mb: parse_mb(&total)?,
    })
}

fn parse_mb(s: &str) -> Option<u32> {
    let trimmed = s.trim().trim_end_matches(" MiB").trim();
    trimmed.parse::<f64>().ok().map(|v| v.round() as u32)
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
