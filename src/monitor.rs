use tokio::sync::mpsc;
use crate::cancellable::Cancellable;
use crate::file::File;

#[derive(Debug)]
pub enum FileMonitorEvent {
    Changed(Box<dyn File>, Option<Box<dyn File>>), // File, OtherFile (for renames)
    ChangesDoneHint(Box<dyn File>),
    Deleted(Box<dyn File>),
    Created(Box<dyn File>),
    AttributeChanged(Box<dyn File>),
    PreUnmount(Box<dyn File>),
    Unmounted(Box<dyn File>),
    Moved(Box<dyn File>, Box<dyn File>), // Src, Dest
}

pub struct FileMonitor {
    receiver: mpsc::Receiver<FileMonitorEvent>,
    _cancellable: Option<Cancellable>,
    _watcher: Option<Box<dyn std::any::Any + Send + Sync>>,
}

impl FileMonitor {
    pub fn new(
        receiver: mpsc::Receiver<FileMonitorEvent>,
        cancellable: Option<Cancellable>,
        watcher: Option<Box<dyn std::any::Any + Send + Sync>>,
    ) -> Self {
        Self {
            receiver,
            _cancellable: cancellable,
            _watcher: watcher,
        }
    }

    pub async fn next_event(&mut self) -> Option<FileMonitorEvent> {
        self.receiver.recv().await
    }
}
