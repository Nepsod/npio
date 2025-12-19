# Integration Guide

This guide explains how to integrate npio into your application, particularly for GUI toolkits like NPTK.

## Basic Setup

### 1. Add Dependency

```toml
[dependencies]
npio = { path = "../npio" }  # or from crates.io when published
tokio = { version = "1", features = ["full"] }
```

### 2. Register Backend

```rust
use npio::{register_backend, get_file_for_uri};
use npio::backend::local::LocalBackend;
use std::sync::Arc;

// Register the local filesystem backend
let backend = Arc::new(LocalBackend::new());
register_backend(backend);
```

## Common Patterns

### File Operations

```rust
use npio::{get_file_for_uri, CopyFlags};
use npio::job;
use tokio::io::AsyncReadExt;

// Get a file handle
let file = get_file_for_uri("file:///home/user/document.txt")?;

// Read file
let mut input = file.read(None).await?;
let mut contents = Vec::new();
input.read_to_end(&mut contents).await?;

// Copy with progress
let dest = get_file_for_uri("file:///home/user/copy.txt")?;
job::copy(
    &*file,
    &*dest,
    CopyFlags::OVERWRITE,
    Some(Box::new(|current, total| {
        println!("Progress: {}/{}", current, total);
    })),
    None,
).await?;
```

### Directory Listing

```rust
use npio::{get_file_for_uri, DirectoryModel, DirectoryUpdate};
use tokio::sync::mpsc;

let dir = get_file_for_uri("file:///home/user")?;
let model = DirectoryModel::new(dir);

// Load directory
model.load(None).await?;

// Subscribe to updates
let mut rx = model.subscribe();

// List files
for file_info in model.files() {
    println!("{}", file_info.get_name().unwrap_or("unknown"));
}

// Handle updates
tokio::spawn(async move {
    while let Ok(update) = rx.recv().await {
        match update {
            DirectoryUpdate::Added(info) => {
                println!("Added: {}", info.get_name().unwrap_or("unknown"));
            }
            DirectoryUpdate::Removed(info) => {
                println!("Removed: {}", info.get_name().unwrap_or("unknown"));
            }
            DirectoryUpdate::Changed(info) => {
                println!("Changed: {}", info.get_name().unwrap_or("unknown"));
            }
            _ => {}
        }
    }
});
```

### File Monitoring

```rust
use npio::{get_file_for_uri, FileMonitorEvent};

let dir = get_file_for_uri("file:///home/user")?;
let mut monitor = dir.monitor(None).await?;

// Listen for events
while let Some(event) = monitor.next_event().await {
    match event {
        FileMonitorEvent::Created(file) => {
            println!("Created: {}", file.basename());
        }
        FileMonitorEvent::Changed(file, _) => {
            println!("Changed: {}", file.basename());
        }
        FileMonitorEvent::Deleted(file) => {
            println!("Deleted: {}", file.basename());
        }
        _ => {}
    }
}
```

### Services

#### Places Service

```rust
use npio::PlacesService;

let service = PlacesService::new();
let places = service.get_common_places();

for place in places {
    println!("{}: {}", place.name, place.file);
}
```

#### Bookmarks Service

```rust
use npio::BookmarksService;

let mut service = BookmarksService::new();
service.load().await?;

// Add bookmark
service.add_bookmark(
    "file:///home/user/Documents".to_string(),
    Some("Documents".to_string()),
);

// Save
service.save().await?;
```

#### Thumbnail Service

```rust
use npio::{ThumbnailService, ThumbnailSize};

let service = ThumbnailService::new();
let file = get_file_for_uri("file:///home/user/image.jpg")?;

// Get thumbnail path if exists
if let Some(path) = service.get_thumbnail_path(&*file, ThumbnailSize::Normal, None).await? {
    println!("Thumbnail: {:?}", path);
}
```

## Integration with GUI Toolkits

### NPTK Integration Example

```rust
use npio::{DirectoryModel, DirectoryUpdate};
use nptk::{ListView, ListItem};

struct FileListModel {
    npio_model: DirectoryModel,
    list_view: ListView,
}

impl FileListModel {
    async fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let dir = get_file_for_uri(&format!("file://{}", path))?;
        let model = DirectoryModel::new(dir);
        model.load(None).await?;
        
        let list_view = ListView::new();
        
        // Populate initial list
        for file_info in model.files() {
            let item = ListItem::new(
                file_info.get_name().unwrap_or("unknown"),
                file_info.get_icon().unwrap_or("text-x-generic"),
            );
            list_view.add_item(item);
        }
        
        // Subscribe to updates
        let mut rx = model.subscribe();
        let list_view_clone = list_view.clone();
        
        tokio::spawn(async move {
            while let Ok(update) = rx.recv().await {
                match update {
                    DirectoryUpdate::Added(info) => {
                        list_view_clone.add_item(ListItem::new(
                            info.get_name().unwrap_or("unknown"),
                            info.get_icon().unwrap_or("text-x-generic"),
                        ));
                    }
                    DirectoryUpdate::Removed(info) => {
                        list_view_clone.remove_item_by_name(
                            info.get_name().unwrap_or("unknown")
                        );
                    }
                    _ => {}
                }
            }
        });
        
        Ok(Self {
            npio_model: model,
            list_view,
        })
    }
}
```

### Progress Reporting

```rust
use npio::job;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

// In your UI component
let progress_bar = ProgressBar::new();
let progress_value = Arc::new(AtomicU64::new(0));
let progress_clone = progress_value.clone();

// Copy with progress
job::copy(
    &*source_file,
    &*dest_file,
    CopyFlags::NONE,
    Some(Box::new(move |current, total| {
        progress_clone.store(current, Ordering::SeqCst);
        // Update UI (this would be done via message passing in real app)
        if total > 0 {
            let percent = (current * 100) / total;
            progress_bar.set_value(percent);
        }
    })),
    None,
).await?;
```

### Cancellation

```rust
use npio::Cancellable;

let cancellable = Cancellable::new();

// Start operation in background
let cancellable_clone = cancellable.clone();
tokio::spawn(async move {
    let file = get_file_for_uri("file:///large/file.bin")?;
    let mut input = file.read(Some(&cancellable_clone)).await?;
    // ... read file
});

// Cancel from UI button
button.on_click(move || {
    cancellable.cancel();
});
```

## Best Practices

1. **Always register backends** before using file operations
2. **Use cancellables** for long-running operations
3. **Subscribe to updates** for reactive UI updates
4. **Handle errors gracefully** - npio uses `NpioResult` for error handling
5. **Use async/await** - all I/O operations are async

## Error Handling

```rust
use npio::{NpioError, IOErrorEnum};

match file.read(None).await {
    Ok(input) => {
        // Handle success
    }
    Err(e) => {
        match e.kind() {
            IOErrorEnum::NotFound => {
                println!("File not found");
            }
            IOErrorEnum::PermissionDenied => {
                println!("Permission denied");
            }
            IOErrorEnum::Cancelled => {
                println!("Operation cancelled");
            }
            _ => {
                println!("Error: {}", e);
            }
        }
    }
}
```

## Performance Tips

1. **Batch operations** when possible
2. **Use progress callbacks** for user feedback on long operations
3. **Monitor selectively** - only monitor directories you need
4. **Cache file info** when displaying large directory lists
5. **Use cancellables** to allow users to cancel long operations

