# npio Architecture

## Overview

npio is a Rust-native Input-Output library inspired by GLib's GIO, providing unified filesystem, device, and I/O abstractions for Linux. This document describes the architecture and how it parallels GIO.

## Core Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Application Layer                     │
│  (Services: Thumbnail, VolumeMonitor, Devices)               │
└───────────────────────┬───────────────────────────────────────┘
                        │
┌───────────────────────▼───────────────────────────────────────┐
│                      API Layer (Traits)                        │
│  File, FileInfo, InputStream, OutputStream, Mount, Volume,    │
│  Drive, FileMonitor, Cancellable, Backend                     │
└───────────────────────┬───────────────────────────────────────┘
                        │
┌───────────────────────▼───────────────────────────────────────┐
│                    Backend Registry                            │
│              Routes URIs to appropriate backends              │
└───────────────────────┬───────────────────────────────────────┘
                        │
        ┌───────────────┼───────────────┐
        │               │               │
┌───────▼──────┐ ┌──────▼──────┐ ┌─────▼──────┐
│ Local FS     │ │ Mount       │ │ Thumbnail  │
│ Backend      │ │ Backend     │ │ Backend    │
│              │ │             │ │            │
│ - file://    │ │ - /proc/    │ │ - XDG      │
│ - tokio::fs  │ │   mountinfo │ │   cache    │
└──────────────┘ └─────────────┘ └────────────┘
```

## GIO Parallels

### File System

| GIO | npio | Description |
|-----|------|-------------|
| `GFile` | `File` trait | URI-based file handle |
| `GFileInfo` | `FileInfo` | Metadata with attribute system |
| `GFileEnumerator` | `FileEnumerator` | Directory enumeration |
| `GInputStream` | `InputStream` | Async read stream |
| `GOutputStream` | `OutputStream` | Async write stream |

### Device Management

| GIO | npio | Description |
|-----|------|-------------|
| `GMount` | `Mount` trait | Mounted filesystem |
| `GVolume` | `Volume` trait | Storage volume |
| `GDrive` | `Drive` trait | Physical drive |

### Services

| GIO | npio | Description |
|-----|------|-------------|
| `GFileMonitor` | `FileMonitor` | File change monitoring |
| `GCancellable` | `Cancellable` | Operation cancellation |
| `GFile` operations | `job` module | Copy, move, delete, trash |

## Component Details

### Backend System

The backend system provides pluggable implementations for different URI schemes:

- **LocalBackend**: Handles `file://` URIs using `tokio::fs`
- **MountBackend**: Parses `/proc/self/mountinfo` for mount information
- **ThumbnailBackend**: Manages freedesktop.org thumbnail cache

### Attribute System

Files have attributes organized by namespace:
- `standard::*` - Name, type, size, icon, etc.
- `time::*` - Modification, access, creation times
- `unix::*` - Unix-specific attributes (mode, uid, gid)
- `thumbnail::*` - Thumbnail paths and validity

### Async Jobs

High-level operations with progress reporting:
- `copy` - Copy files with progress callbacks
- `move_` - Move/rename files
- `delete` - Delete files
- `trash` - Move files to trash (freedesktop.org spec)

### Services

High-level services built on top of the core API:
- **ThumbnailService**: Thumbnail generation and caching
- **VolumeMonitor**: Device and volume monitoring
- **DevicesModel**: Unified view of drives, volumes, mounts

## Data Flow

### File Operations

```
Application
    │
    ├─► get_file_for_uri("file:///path")
    │       │
    │       └─► BackendRegistry
    │               │
    │               └─► LocalBackend
    │                       │
    │                       └─► LocalFile
    │
    └─► file.read()
            │
            └─► LocalFile::read()
                    │
                    └─► tokio::fs::File::open()
```

### Mount Operations

```
Application
    │
    ├─► MountBackend::get_mounts()
    │       │
    │       └─► Read /proc/self/mountinfo
    │               │
    │               └─► Parse mount entries
    │                       │
    │                       └─► Create UnixMount instances
    │
    └─► mount.get_root()
            │
            └─► Return File handle for mount point
```

## Future Enhancements

### UDisks2 Integration

The Devices Model will integrate with UDisks2 via D-Bus to provide:
- Real drive enumeration
- Volume management
- Hotplug detection via udev
- Eject/mount operations

### Thumbnailer Integration

The Thumbnail Service will:
- Parse `.thumbnailer` files from `~/.local/share/thumbnailers/`
- Invoke appropriate thumbnailers (e.g., gst-thumbnailers)
- Support various MIME types (images, PDF, video, etc.)

## Design Principles

1. **Async-First**: All I/O operations are async using Tokio
2. **Trait-Based**: Core abstractions are traits for flexibility
3. **Backend Pluggable**: New backends can be added easily
4. **GIO-Compatible**: API mirrors GIO where possible
5. **Linux-Focused**: Optimized for Linux filesystem features

