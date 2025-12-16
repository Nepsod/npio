use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use crate::cancellable::Cancellable;
use crate::error::NpioResult;
use crate::file::File;
use crate::file_info::FileInfo;
use crate::monitor::FileMonitorEvent;

#[derive(Debug, Clone)]
pub enum DirectoryUpdate {
    Initial(Vec<FileInfo>),
    Added(FileInfo),
    Removed(FileInfo),
    Changed(FileInfo),
}

pub struct DirectoryModel {
    file: Box<dyn File>,
    files: Arc<RwLock<Vec<FileInfo>>>,
    update_tx: broadcast::Sender<DirectoryUpdate>,
}

impl DirectoryModel {
    pub fn new(file: Box<dyn File>) -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            file,
            files: Arc::new(RwLock::new(Vec::new())),
            update_tx: tx,
        }
    }

    pub fn files(&self) -> Vec<FileInfo> {
        self.files.read().unwrap().clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DirectoryUpdate> {
        self.update_tx.subscribe()
    }

    pub async fn load(&self, cancellable: Option<&Cancellable>) -> NpioResult<()> {
        if let Some(c) = cancellable {
            c.check()?;
        }

        // 1. Enumerate existing files
        let mut enumerator = self.file.enumerate_children("standard::*,time::modified", cancellable).await?;
        let mut initial_files = Vec::new();

        while let Some((info, _child)) = enumerator.next_file(cancellable).await? {
            initial_files.push(info);
        }
        enumerator.close(cancellable).await?;

        // Update state
        {
            let mut files = self.files.write().unwrap();
            *files = initial_files.clone();
        }

        // Notify initial
        let _ = self.update_tx.send(DirectoryUpdate::Initial(initial_files));

        // 2. Start monitoring
        let mut monitor = self.file.monitor(cancellable).await?;
        let files_clone = self.files.clone();
        let tx_clone = self.update_tx.clone();
        
        // Spawn monitoring task
        // Note: This task will run until the monitor is dropped or channel closed.
        // Since `monitor` is owned by the task, it will be dropped when the task finishes.
        // The task finishes when `next_event` returns None (channel closed).
        tokio::spawn(async move {
            while let Some(event) = monitor.next_event().await {
                match event {
                    FileMonitorEvent::Created(child) => {
                        // Query info for new file
                        if let Ok(info) = child.query_info("standard::*,time::modified", None).await {
                            {
                                let mut files = files_clone.write().unwrap();
                                files.push(info.clone());
                            }
                            let _ = tx_clone.send(DirectoryUpdate::Added(info));
                        }
                    },
                    FileMonitorEvent::Deleted(child) => {
                        let basename = child.basename();
                        let mut removed_info = None;
                        {
                            let mut files = files_clone.write().unwrap();
                            if let Some(pos) = files.iter().position(|f| f.get_name() == Some(&basename)) {
                                removed_info = Some(files.remove(pos));
                            }
                        }
                        if let Some(info) = removed_info {
                            let _ = tx_clone.send(DirectoryUpdate::Removed(info));
                        }
                    },
                    FileMonitorEvent::Changed(child, _) => {
                        if let Ok(info) = child.query_info("standard::*,time::modified", None).await {
                             let basename = child.basename();
                             {
                                let mut files = files_clone.write().unwrap();
                                if let Some(pos) = files.iter().position(|f| f.get_name() == Some(&basename)) {
                                    files[pos] = info.clone();
                                }
                            }
                            let _ = tx_clone.send(DirectoryUpdate::Changed(info));
                        }
                    },
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
