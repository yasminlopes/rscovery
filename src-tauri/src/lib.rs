use serde::Serialize;
use sysinfo::Disks;

mod analyze_blocks;
mod find_file;

#[derive(Debug, Serialize)]
pub struct DiskInfo {
    name: String,
    mount_point: String,
    device_path: String,
    size: u64,
}


#[tauri::command]
fn list_disks() -> Vec<DiskInfo> {
    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh_list();

    disks
        .iter()
        .map(|disk| {
            let mount = disk.mount_point().to_string_lossy().to_string();
            let size_mb = disk.total_space() / 1024 / 1024;
            let disk_name = disk.name().to_string_lossy().to_string();

            #[cfg(target_os = "linux")]
            let device_path = {
                use std::fs::File;
                use std::io::{BufRead, BufReader};

                let mut device = mount.clone();
                if let Ok(f) = File::open("/proc/mounts") {
                    let reader = BufReader::new(f);
                    for line in reader.lines().filter_map(Result::ok) {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        if parts.len() >= 2 && parts[1] == mount {
                            device = parts[0].to_string();
                            break;
                        }
                    }
                }
                device
            };

            #[cfg(target_os = "macos")]
            let device_path = {
                use std::process::Command;
                
                let mut device = String::new();
                
                // Usar df para descobrir o device do mount point
                if let Ok(output) = Command::new("df").arg(&mount).output() {
                    let output_str = String::from_utf8_lossy(&output.stdout);
                    // A segunda linha contém o device
                    if let Some(line) = output_str.lines().nth(1) {
                        if let Some(dev) = line.split_whitespace().next() {
                            device = dev.to_string();
                        }
                    }
                }
                
                // Se não conseguiu, tentar diskutil
                if device.is_empty() {
                    if let Ok(output) = Command::new("diskutil")
                        .arg("info")
                        .arg(&mount)
                        .output()
                    {
                        let output_str = String::from_utf8_lossy(&output.stdout);
                        for line in output_str.lines() {
                            if line.contains("Device Node:") {
                                if let Some(dev) = line.split(':').nth(1) {
                                    device = dev.trim().to_string();
                                    break;
                                }
                            }
                        }
                    }
                }
                
                // Se ainda está vazio, usar o mount point como fallback
                if device.is_empty() {
                    device = mount.clone();
                }
                
                device
            };

            #[cfg(not(any(target_os = "linux", target_os = "macos")))]
            let device_path = mount.clone();

            println!("Disk found: name='{}', mount='{}', device='{}'", disk_name, mount, device_path);

            DiskInfo {
                name: disk_name,
                mount_point: mount,
                device_path,
                size: size_mb,
            }
        })
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_disks,
            analyze_blocks::analyze_blocks,
            find_file::find_jpeg,
            find_file::find_png,
            find_file::find_pdf,
            find_file::find_zip,
            find_file::find_txt,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}