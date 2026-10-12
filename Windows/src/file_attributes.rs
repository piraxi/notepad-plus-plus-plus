//! Metadata reads run off the UI thread: remote paths must not freeze editing.
use npp_editor_core::{BufferId, DiskFingerprint};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{PostMessageW, WM_APP},
};

pub const READY: u32 = WM_APP + 41;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiskState {
    Unchanged,
    Changed,
    Deleted,
    Unavailable,
}
pub struct AttributeState {
    pub id: BufferId,
    pub path: PathBuf,
    pub read_only: Option<bool>,
    pub disk: DiskState,
}

pub struct AttributeCheck {
    receiver: Receiver<Vec<AttributeState>>,
    cancelled: Arc<AtomicBool>,
}
impl AttributeCheck {
    pub fn try_recv(&self) -> Result<Vec<AttributeState>, mpsc::TryRecvError> {
        self.receiver.try_recv()
    }
    #[cfg(test)]
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<Vec<AttributeState>, mpsc::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}
impl Drop for AttributeCheck {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

pub fn start(
    window: HWND,
    paths: Vec<(BufferId, PathBuf, Option<DiskFingerprint>)>,
) -> AttributeCheck {
    let (sender, receiver) = mpsc::channel();
    let window = window as usize;
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = cancelled.clone();
    std::thread::spawn(move || {
        let states = paths
            .into_iter()
            .take_while(|_| !worker_cancelled.load(Ordering::Relaxed))
            .map(|(id, path, baseline)| {
                let (read_only, disk) = match std::fs::metadata(&path) {
                    Ok(metadata) if metadata.is_file() => {
                        let disk = match DiskFingerprint::read_cancellable(&path, || {
                            worker_cancelled.load(Ordering::Relaxed)
                        }) {
                            Ok(current) if baseline.is_some_and(|saved| saved != current) => {
                                DiskState::Changed
                            }
                            Ok(_) => DiskState::Unchanged,
                            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                                DiskState::Deleted
                            }
                            Err(_) => DiskState::Unavailable,
                        };
                        (Some(metadata.permissions().readonly()), disk)
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        (None, DiskState::Deleted)
                    }
                    _ => (None, DiskState::Unavailable),
                };
                AttributeState {
                    id,
                    path,
                    read_only,
                    disk,
                }
            })
            .collect();
        if !worker_cancelled.load(Ordering::Relaxed) && sender.send(states).is_ok() {
            // SAFETY: No pointer payload or borrowed application state crosses
            // threads. A stale notification is harmless; the receiver owns data.
            unsafe {
                PostMessageW(window as HWND, READY, 0, 0);
            }
        }
    });
    AttributeCheck {
        receiver,
        cancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropping_a_check_signals_worker_cancellation() {
        let (_, receiver) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        let check = AttributeCheck {
            receiver,
            cancelled: cancelled.clone(),
        };
        assert!(!cancelled.load(Ordering::Relaxed));
        drop(check);
        assert!(cancelled.load(Ordering::Relaxed));
    }
}
