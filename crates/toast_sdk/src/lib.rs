use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u64,
    pub kind: ToastKind,
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

#[derive(Clone, Debug)]
pub struct ToastRequest {
    pub kind: ToastKind,
    pub message: String,
    pub duration: Duration,
}

#[derive(Default)]
pub struct ToastManager {
    next_id: u64,
    active: Vec<Toast>,
}

impl ToastManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> &[Toast] {
        &self.active
    }

    pub fn push(&mut self, request: ToastRequest) -> u64 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        self.active.push(Toast {
            id,
            kind: request.kind,
            message: request.message,
            created_at: Instant::now(),
            duration: request.duration,
        });
        id
    }

    pub fn dismiss(&mut self, id: u64) {
        self.active.retain(|toast| toast.id != id);
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        self.active
            .retain(|toast| now.duration_since(toast.created_at) < toast.duration);
    }

    pub fn ingest_pending(&mut self) {
        for request in drain_pending() {
            self.push(request);
        }
    }
}

const DEFAULT_TOAST_DURATION: Duration = Duration::from_millis(3000);

static TOAST_QUEUE: OnceLock<Mutex<Vec<ToastRequest>>> = OnceLock::new();

fn queue() -> &'static Mutex<Vec<ToastRequest>> {
    TOAST_QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn enqueue_toast(kind: ToastKind, message: impl Into<String>, duration: Option<Duration>) {
    let request = ToastRequest {
        kind,
        message: message.into(),
        duration: duration.unwrap_or(DEFAULT_TOAST_DURATION),
    };

    let mut queue = queue().lock().expect("toast queue lock poisoned");
    queue.push(request);
}

pub fn drain_pending() -> Vec<ToastRequest> {
    let mut queue = queue().lock().expect("toast queue lock poisoned");
    std::mem::take(&mut *queue)
}

pub fn info(message: impl Into<String>) {
    enqueue_toast(ToastKind::Info, message, None);
}

pub fn success(message: impl Into<String>) {
    enqueue_toast(ToastKind::Success, message, None);
}

pub fn warning(message: impl Into<String>) {
    enqueue_toast(ToastKind::Warning, message, None);
}

pub fn error(message: impl Into<String>) {
    enqueue_toast(ToastKind::Error, message, None);
}
