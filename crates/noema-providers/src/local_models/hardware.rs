//! Conservative, platform-aware local inference hardware detection.

use std::{io, process::Command};

#[cfg(target_os = "linux")]
use std::fs;

use thiserror::Error;

use super::LocalHardwareProfile;
use crate::LocalModelBackend;

const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
#[cfg(target_os = "linux")]
const KIB_PER_GIB: u64 = 1024 * 1024;

/// Failures while reading the machine values used by catalog selection.
#[derive(Debug, Error)]
pub enum LocalHardwareProbeError {
    /// The platform's physical-memory value could not be read.
    #[error("failed to detect physical memory: {0}")]
    MemoryProbe(#[source] io::Error),
    /// The platform returned an unusable physical-memory value.
    #[error("physical memory probe returned an invalid value: {0}")]
    InvalidMemory(String),
}

/// Detect usable local inference backends in platform preference order.
///
/// Accelerator profiles are reported conservatively: a discrete backend that
/// cannot provide a VRAM value remains detectable, but catalog builds with a
/// VRAM threshold will correctly decline to match it. CPU is always the last
/// fallback.
///
/// # Errors
///
/// Returns [`LocalHardwareProbeError`] when total physical memory cannot be
/// detected, since every catalog build depends on that value.
pub fn detect_local_hardware_profiles() -> Result<Vec<LocalHardwareProfile>, LocalHardwareProbeError>
{
    let ram_gb = detect_ram_gb()?;
    let mut profiles = Vec::new();

    #[cfg(target_os = "macos")]
    {
        let unified = cfg!(target_arch = "aarch64");
        let vram_gb = if unified { None } else { macos_vram_gb() };
        profiles.push(LocalHardwareProfile::new(
            LocalModelBackend::Metal,
            ram_gb,
            vram_gb,
            unified,
        ));
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(cuda_vram_gb) = nvidia_vram_gb() {
            profiles.push(LocalHardwareProfile::new(
                LocalModelBackend::Cuda,
                ram_gb,
                Some(cuda_vram_gb),
                false,
            ));
        }
        if windows_vulkan_available() {
            profiles.push(LocalHardwareProfile::new(
                LocalModelBackend::Vulkan,
                ram_gb,
                windows_vram_gb(),
                false,
            ));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if linux_vulkan_available() {
            profiles.push(LocalHardwareProfile::new(
                LocalModelBackend::Vulkan,
                ram_gb,
                linux_vram_gb(),
                false,
            ));
        }
    }

    profiles.push(LocalHardwareProfile::new(
        LocalModelBackend::Cpu,
        ram_gb,
        None,
        false,
    ));
    Ok(profiles)
}

#[cfg(target_os = "linux")]
pub(super) fn detect_ram_gb() -> Result<u64, LocalHardwareProbeError> {
    let meminfo =
        fs::read_to_string("/proc/meminfo").map_err(LocalHardwareProbeError::MemoryProbe)?;
    parse_linux_mem_total_gb(&meminfo)
        .ok_or_else(|| LocalHardwareProbeError::InvalidMemory("/proc/meminfo MemTotal".to_string()))
}

#[cfg(target_os = "macos")]
pub(super) fn detect_ram_gb() -> Result<u64, LocalHardwareProbeError> {
    let output = command_stdout("sysctl", &["-n", "hw.memsize"])
        .map_err(LocalHardwareProbeError::MemoryProbe)?;
    parse_bytes_gb(&output)
        .ok_or_else(|| LocalHardwareProbeError::InvalidMemory("sysctl hw.memsize".to_string()))
}

#[cfg(target_os = "windows")]
pub(super) fn detect_ram_gb() -> Result<u64, LocalHardwareProbeError> {
    let output = command_stdout(
        "powershell.exe",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
        ],
    )
    .map_err(LocalHardwareProbeError::MemoryProbe)?;
    parse_bytes_gb(&output).ok_or_else(|| {
        LocalHardwareProbeError::InvalidMemory(
            "Win32_ComputerSystem.TotalPhysicalMemory".to_string(),
        )
    })
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(super) fn detect_ram_gb() -> Result<u64, LocalHardwareProbeError> {
    Err(LocalHardwareProbeError::InvalidMemory(
        "unsupported operating system".to_string(),
    ))
}

#[cfg(target_os = "linux")]
fn parse_linux_mem_total_gb(meminfo: &str) -> Option<u64> {
    let kib = meminfo.lines().find_map(|line| {
        let value = line.strip_prefix("MemTotal:")?.trim();
        value.strip_suffix("kB")?.trim().parse::<u64>().ok()
    })?;
    whole_gib(kib, KIB_PER_GIB)
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn parse_bytes_gb(value: &str) -> Option<u64> {
    let bytes = value.trim().parse::<u64>().ok()?;
    whole_gib(bytes, BYTES_PER_GIB)
}

fn whole_gib(value: u64, units_per_gib: u64) -> Option<u64> {
    let gib = value / units_per_gib;
    (gib > 0).then_some(gib)
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn nvidia_vram_gb() -> Option<u64> {
    let output = command_stdout(
        "nvidia-smi",
        &["--query-gpu=memory.total", "--format=csv,noheader,nounits"],
    )
    .ok()?;
    output
        .lines()
        .filter_map(|line| line.trim().parse::<u64>().ok())
        .max()
        .and_then(|mib| whole_gib(mib, 1024))
}

#[cfg(target_os = "linux")]
fn linux_vulkan_available() -> bool {
    command_stdout("vulkaninfo", &["--summary"]).is_ok()
        || [
            "/usr/lib/libvulkan.so.1",
            "/usr/lib64/libvulkan.so.1",
            "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
            "/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
        ]
        .iter()
        .any(|path| std::path::Path::new(path).is_file())
}

#[cfg(target_os = "linux")]
fn linux_vram_gb() -> Option<u64> {
    let sysfs_vram = fs::read_dir("/sys/class/drm")
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            fs::read_to_string(entry.path().join("device/mem_info_vram_total"))
                .ok()?
                .trim()
                .parse::<u64>()
                .ok()
        })
        .max()
        .and_then(|bytes| whole_gib(bytes, BYTES_PER_GIB));
    sysfs_vram.or_else(nvidia_vram_gb)
}

#[cfg(target_os = "windows")]
fn windows_vulkan_available() -> bool {
    std::env::var_os("WINDIR").is_some_and(|windows| {
        std::path::PathBuf::from(windows)
            .join("System32/vulkan-1.dll")
            .is_file()
    }) || command_stdout("vulkaninfo.exe", &["--summary"]).is_ok()
}

#[cfg(target_os = "windows")]
fn windows_vram_gb() -> Option<u64> {
    nvidia_vram_gb().or_else(|| {
        let output = command_stdout(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_VideoController | ForEach-Object AdapterRAM | Sort-Object -Descending | Select-Object -First 1",
            ],
        )
        .ok()?;
        parse_bytes_gb(&output)
    })
}

#[cfg(target_os = "macos")]
fn macos_vram_gb() -> Option<u64> {
    let output = command_stdout("system_profiler", &["SPDisplaysDataType"]).ok()?;
    output.lines().filter_map(parse_macos_vram_line).max()
}

#[cfg(target_os = "macos")]
fn parse_macos_vram_line(line: &str) -> Option<u64> {
    if !line.contains("VRAM") {
        return None;
    }
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    tokens.windows(2).find_map(|pair| {
        let amount = pair[0].parse::<u64>().ok()?;
        match pair[1].trim_end_matches(',') {
            "GB" => Some(amount),
            "MB" => whole_gib(amount, 1024),
            _ => None,
        }
    })
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn command_stdout(program: &str, arguments: &[&str]) -> io::Result<String> {
    let output = Command::new(program).args(arguments).output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "{program} exited with {}",
            output.status
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn parses_linux_memory_in_whole_gibibytes() {
        assert_eq!(
            parse_linux_mem_total_gb("MemTotal:       16777216 kB\nMemFree: 1 kB\n"),
            Some(16)
        );
        assert_eq!(parse_linux_mem_total_gb("MemFree: 1 kB\n"), None);
    }

    #[test]
    fn whole_gib_is_conservative() {
        assert_eq!(whole_gib(BYTES_PER_GIB * 12 + 999, BYTES_PER_GIB), Some(12));
        assert_eq!(whole_gib(BYTES_PER_GIB - 1, BYTES_PER_GIB), None);
    }

    #[test]
    fn detected_profiles_always_end_with_cpu() {
        let profiles = detect_local_hardware_profiles().expect("hardware profile");
        assert_eq!(
            profiles.last().map(|profile| profile.backend),
            Some(LocalModelBackend::Cpu)
        );
        assert!(profiles.last().is_some_and(|profile| profile.ram_gb > 0));
    }
}
