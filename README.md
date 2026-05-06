# npio - Nepsod Input-Output

[![License](https://img.shields.io/badge/license-LGPL--3.0--or--later-blue.svg)](LICENSE.txt)

GIO-inspired Rust library for Linux filesystem and device I/O. Provides async, URI-based file operations, directory monitoring, thumbnails, and volume management.

## Features

- **URI-based filesystem abstraction** - Unified API for local and remote files
- **Async-first** - Built on Tokio for high-performance I/O
- **Directory monitoring** - Real-time file system change notifications
- **Thumbnail service** - Automatic thumbnail generation and caching
- **Volume monitoring** - Device and mount point tracking
- **Pluggable backends** - Extensible architecture for different URI schemes

## Usage

```toml
[dependencies]
npio = { path = "../npio" }
tokio = { version = "1", features = ["full"] }
```

```rust
use npio::{get_file_for_uri, register_backend};
use npio::backend::local::LocalBackend;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    register_backend(Arc::new(LocalBackend::new()));

    let file = get_file_for_uri("file:///home/user/document.txt")?;
    let info = file.query_info("standard::*,time::modified", None).await?;
    println!("{}: {} bytes", info.get_name().unwrap_or("unknown"), info.get_size());
    
    Ok(())
}
```

## Core API

**File Operations**
```rust
let file = get_file_for_uri("file:///path/to/file")?;
file.copy(&dest, CopyFlags::OVERWRITE, None, None).await?;
file.trash(None).await?;
```

**Directory Monitoring**
```rust
let model = DirectoryModel::new(file);
model.load(None).await?;
let mut rx = model.subscribe();
while let Ok(update) = rx.recv().await {
    match update {
        DirectoryUpdate::Added(info) => println!("Added: {}", info.get_name().unwrap_or("unknown")),
        _ => {}
    }
}
```

**Thumbnails**
```rust
let service = ThumbnailService::new();
if let Some(path) = service.get_thumbnail_path(&*file, ThumbnailSize::Normal, None).await? {
    println!("Thumbnail: {:?}", path);
}
```

**Volume Monitoring**
```rust
let monitor = VolumeMonitor::new();
monitor.start(None).await?;
let mut rx = monitor.subscribe();
while let Ok(event) = rx.recv().await {
    match event {
        VolumeMonitorEvent::MountAdded { mount } => println!("Mounted: {}", mount),
        _ => {}
    }
}
```

## Backends

- `LocalBackend` - Local filesystem operations
- `MountBackend` - Mount point management
- `UDisks2Backend` - Device management via UDisks2
- `ThumbnailBackend` - Thumbnail generation

## Examples

```bash
cargo run --example file_copy -- /path/to/source /path/to/dest
cargo run --example directory_listing -- /path/to/directory
```

## Documentation

- [Architecture Guide](docs/ARCHITECTURE.md)
- [Integration Guide](docs/INTEGRATION.md)

## License

LGPL-3.0-or-later - see [LICENSE.txt](LICENSE.txt)
