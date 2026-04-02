import { Link, useLocation } from "react-router-dom";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ScannedData {
  current: number;
  total: number;
  non_empty: number[]; // Corrigido de nonEmpty para non_empty (snake_case do Rust)
}

interface DiskInfo {
  name: string;
  mount_point: string;
  device_path: string;
  size: number;
}

export default function Blocks() {
  const { search } = useLocation();
  const queryParams = new URLSearchParams(search);

  const diskName = queryParams.get("id"); // Nome do disco vindo da URL

  const [disk, setDisk] = useState<DiskInfo | null>(null);
  const [loadingScan, setLoadingScan] = useState(false);
  const [scannedSize, setScannedSize] = useState<ScannedData>({
    current: 0,
    total: 0,
    non_empty: [],
  });

  // Buscar informações do disco
  useEffect(() => {
    const fetchDiskInfo = async () => {
      try {
        const disks = await invoke<DiskInfo[]>("list_disks");
        console.log("All disks:", disks);
        
        // Encontrar o disco pelo nome
        const foundDisk = disks.find((d) => d.name === diskName);
        if (foundDisk) {
          console.log("Selected disk:", foundDisk);
          setDisk(foundDisk);
        } else {
          console.error("Disk not found:", diskName);
        }
      } catch (err) {
        console.error("Error fetching disks:", err);
      }
    };

    fetchDiskInfo();
  }, [diskName]);

  // nonEmpty shows the blocks index (the size is 32MB) that are non empty
  // we should convert it to the actual index.
  const viewBlockSize = scannedSize.total / 200;
  const nonEmptyBlocks = scannedSize.non_empty.map((index) => (index + 1) * 32);
  console.log({ nonEmptyBlocks });

  useEffect(() => {
    const unlistenProgress = listen<ScannedData>("scan-progress", (event) => {
      const progress = event.payload;
      console.log("Progress:", progress);
      setScannedSize({
        current: progress.current,
        total: progress.total,
        non_empty: progress.non_empty, // Corrigido
      });
    });

    return () => {
      unlistenProgress.then((f) => f());
    };
  }, []);

  const handleStartScan = async () => {
    if (!disk) {
      console.error("No disk selected");
      return;
    }

    try {
      console.log("Starting scan on device:", disk.device_path);
      setLoadingScan(true);
      // Usar device_path em vez do nome
      await invoke("analyze_blocks", { path: disk.device_path });
      setLoadingScan(false);
    } catch (err) {
      console.error("Error starting scan:", err);
      setLoadingScan(false);
    }
  };

  if (!disk) {
    return (
      <main className="container">
        <header>
          <div>
            <Link to={"/"}>Go Back</Link>
          </div>
          <h1>Loading disk information...</h1>
        </header>
      </main>
    );
  }

  return (
    <main className="container">
      <header>
        <div>
          <Link to={"/"}>Go Back</Link>
        </div>
        <h1>Disk "{disk.name}"</h1>
        <p style={{ fontSize: "0.9em", color: "#666" }}>
          Device: {disk.device_path} | Mount: {disk.mount_point}
        </p>
      </header>

      {loadingScan ? (
        <div>
          <p>
            {(scannedSize.current / 1024).toFixed(2)}/
            {(scannedSize.total / 1024).toFixed(2)} GB
          </p>
          <div className="scanGrid">
            {Array.from({ length: 200 }, (_, it) => {
              const i = it + 2;
              const cellRange = [
                Math.max(0, i - 1) * viewBlockSize,
                (i + 1) * viewBlockSize,
              ];
              const nonEmptyCell = nonEmptyBlocks.find(
                (value) => value >= cellRange[0] && value <= cellRange[1]
              );

              if (!nonEmptyCell) {
                console.log({ cellRange, nonEmptyBlocks });
              }

              return (
                <div
                  style={
                    nonEmptyCell
                      ? { backgroundColor: "rgb(142, 255, 168)" }
                      : scannedSize.current / scannedSize.total > i / 200
                      ? { backgroundColor: "rgb(116, 114, 114)" }
                      : {}
                  }
                  key={i}
                />
              );
            })}
          </div>
        </div>
      ) : (
        <div style={{ marginTop: "24px", textAlign: "center" }}>
          <button onClick={handleStartScan}>Start Scan</button>
        </div>
      )}
    </main>
  );
}