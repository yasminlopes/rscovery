use std::fs::File;
use std::io::Read;
use tauri::Emitter;

use serde::Serialize;

#[derive(Serialize, Clone)]
struct Progress {
    non_empty: Vec<i32>,
    total: f64,
    current: f64,
}


/// Devices like /dev/sdc1 (block devices) are special files, they represent
/// a block device (i.e. a hardware abstraction). The kernel does not store
/// their size in the filesystem metadata. That's why using `metadata().len()` 
/// does not work. 
/// We need to find the number of 512byte sectors to find the size of the device.
pub fn get_block_device_size_gb(device: &str) -> std::io::Result<f64> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        
        let path = format!("/sys/class/block/{}/size", device.replace("/dev/", ""));
        let blocks: u64 = fs::read_to_string(path)?
            .trim()
            .parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let bytes = blocks * 512; 
        let gb = bytes as f64 / 1024.0 / 1024.0;
        Ok(gb)
    }

    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::fs;
        
        // Primeiro, tentar obter o tamanho usando metadata (funciona para arquivos de imagem)
        if let Ok(metadata) = fs::metadata(device) {
            let gb = metadata.len() as f64 / 1024.0 / 1024.0;
            if gb > 0.0 {
                return Ok(gb);
            }
        }
        
        // Se não funcionar, tentar diskutil para block devices
        let output = Command::new("diskutil")
            .arg("info")
            .arg(device)
            .output()?;

        if !output.status.success() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to get disk info for {}", device)
            ));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Procurar pela linha "Disk Size" ou "Total Size"
        for line in output_str.lines() {
            if line.contains("Disk Size") || line.contains("Total Size") {
                // Formato: "   Disk Size:              500.1 GB (500107862016 Bytes) (exactly 976773168 512-Byte-Units)"
                if let Some(bytes_part) = line.split('(').nth(1) {
                    if let Some(bytes_str) = bytes_part.split_whitespace().next() {
                        if let Ok(bytes) = bytes_str.parse::<u64>() {
                            let gb = bytes as f64 / 1024.0 / 1024.0;
                            return Ok(gb);
                        }
                    }
                }
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Could not parse disk size for {}", device)
        ))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        use std::fs;
        
        // Fallback: tentar usar metadata
        let metadata = fs::metadata(device)?;
        let gb = metadata.len() as f64 / 1024.0 / 1024.0;
        Ok(gb)
    }
}

#[tauri::command]
pub async fn analyze_blocks(app_handle: tauri::AppHandle, path: &str) -> Result<(), String> {
    println!("Starting analysis of: {}", path);
    
    // Tentar desmontar primeiro no macOS
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        
        println!("Attempting to unmount device: {}", path);
        
        // Tentar desmontar a partição
        let output = Command::new("diskutil")
            .arg("unmount")
            .arg(path)
            .output();
            
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                println!("Unmount output: {}", stdout);
                if !stderr.is_empty() {
                    println!("Unmount stderr: {}", stderr);
                }
            }
            Err(e) => {
                println!("Warning: Could not unmount device: {}", e);
            }
        }
        
        // Aguardar um pouco para o sistema liberar o recurso
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
    
    let mut file = File::open(path).map_err(|e| {
        let err_msg = format!("Failed to open file {}: {}", path, e);
        println!("{}", err_msg);
        err_msg
    })?;
    
    let total_size = get_block_device_size_gb(path).map_err(|e| {
        let err_msg = format!("Failed to get device size: {}", e);
        println!("{}", err_msg);
        err_msg
    })?;

    println!("Total size: {:.2} MB", total_size);

    let mut buffer = vec![0u8; 32 * 1024 * 1024];
    let mut total_read: u64 = 0;

    let mut iteration = 0;
    let mut non_empty: Vec<i32> = Vec::new();

    loop {
        let bytes_read = file.read(&mut buffer).map_err(|e| {
            let err_msg = format!("Failed to read: {}", e);
            println!("{}", err_msg);
            err_msg
        })?;
        
        if bytes_read == 0 {
            break;
        }
        total_read += bytes_read as u64;
        
        let valid_bytes = buffer[..bytes_read].iter().filter(|&&b| b != 0x00).count();
        if valid_bytes > 0 {
            non_empty.push(iteration);
            println!("Block {} has {} non-zero bytes", iteration, valid_bytes);
        }
        iteration += 1;

        let progress = Progress {
            non_empty: non_empty.clone(),
            total: total_size,
            current: total_read as f64 / 1024.0 / 1024.0,
        };

        println!("Progress: {:.2} MB / {:.2} MB", progress.current, progress.total);
        app_handle.emit("scan-progress", progress).unwrap();
    }

    println!("Reading completed. Total blocks: {}, Non-empty: {}", iteration, non_empty.len());
    Ok(())
}