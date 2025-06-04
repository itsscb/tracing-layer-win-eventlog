use std::collections::HashMap;
use tracing::field::Visit;
use tracing::{Level, Subscriber};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::EventLog::{DeregisterEventSource, RegisterEventSourceW, ReportEventW, EVENTLOG_ERROR_TYPE, EVENTLOG_INFORMATION_TYPE, EVENTLOG_WARNING_TYPE};

/// Wrapper to mark the HANDLE as Send & Sync
#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
struct EventSourceHandle {
    hwnd: *mut std::ffi::c_void,
}
unsafe impl Send for EventSourceHandle {}
unsafe impl Sync for EventSourceHandle {}

impl From<EventSourceHandle> for HANDLE {
    fn from(value: EventSourceHandle) -> Self {
         Self(value.hwnd)
    }
}

impl From<HANDLE> for EventSourceHandle {
    fn from(value: HANDLE) -> Self {
         Self{ hwnd: value.0 }
    }
}

pub fn write_to_event_log(event_source: HANDLE, event_id: u32, level: Level, message: &str) {
    let event_type = match level {
        Level::ERROR => EVENTLOG_ERROR_TYPE,
        Level::WARN => EVENTLOG_WARNING_TYPE,
        Level::INFO | Level::DEBUG | Level::TRACE => EVENTLOG_INFORMATION_TYPE,
    };

    let message = HSTRING::from(message);
    if let Err(e) = unsafe {
        ReportEventW(
            event_source,
            event_type,
            0,
            event_id,
            None,
            0,
            Some(&[PCWSTR(message.as_ptr())]),
            None,
        )
    } {
        eprintln!("Failed to write to event log: {:?}", e);
    };
}

pub struct EventLogLayer {
    event_source: EventSourceHandle,
    default_id: Option<u32>
}

impl Drop for EventLogLayer {
    fn drop(&mut self) {
        let _ = unsafe { DeregisterEventSource(self.event_source.into()) };
    }
}

impl EventLogLayer {
    pub fn new(log_name: &str) -> Result<Self, windows_result::Error> {
        Self::new_with_default_id(log_name, None)
    }

    pub fn new_with_default_id(log_name: &str, default_id: Option<u32>) -> Result<Self, windows_result::Error> {
        let log_name = HSTRING::from(log_name);
        let Ok(event_source) = (unsafe {
            RegisterEventSourceW(
                None,
                PCWSTR(log_name.as_ptr())
            )
        }) else {
            return Err(windows_result::Error::from_win32());
        };
        Ok(Self { event_source: event_source.into(), default_id })
    }
}
impl<S> Layer<S> for EventLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = EventVisitor {
            event_source: self.event_source.into(),
            default_id: self.default_id,
            id: None,
            message: None,
            parents: None,
            log_level: *metadata.level(),
            fields: HashMap::new(),
        };

        event.record(&mut visitor);

        let mut parents = Vec::new();

        let span = ctx.lookup_current().map(|s| {
            let mut current_span = s;
            while let Some(span) = current_span.parent() {
                parents.push(span.name().to_owned());

                current_span = span;
            }
            current_span.name().to_owned()
        });

        if parents.is_empty() {
            visitor.parents = span;
        } else {
            visitor.parents = Some(
                parents
                    .into_iter()
                    .rev()
                    .collect::<Vec<String>>()
                    .join(" / "),
            );
        }

        visitor.log();
    }
}

#[derive(Debug)]
struct EventVisitor {
    event_source: HANDLE,
    default_id: Option<u32>,
    id: Option<u32>,
    log_level: Level,
    message: Option<String>,
    parents: Option<String>,
    fields: HashMap<String, String>,
}

impl EventVisitor {
    fn log(&self) {
        let id: u32 = self.id.unwrap_or(self.default_id.unwrap_or_default());

        let mut msg = String::new();
        
        if let Some(m) = &self.message {
            msg.push_str(&format!("{m}\n\n"));
        }

        if let Some(m) = &self.parents {
            msg.push_str(&format!("source: {m}\n"));
        }
        self.fields.iter().for_each(|i| {
            msg.push_str(&format!("{}: {:?}\n", i.0, i.1.replace(r"\\", r"\")));
        });

        write_to_event_log(self.event_source, id, self.log_level, &msg);
    }
}

impl Visit for EventVisitor {
    #[allow(clippy::cast_possible_truncation)]
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name().to_lowercase() == "id" && value <= u32::MAX.into() {
            self.id = Some(value as u32);
        } else {
            self.record_debug(field, &value);
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name().to_lowercase() == "id" && value >= 0 && value <= u32::MAX.into() {
            self.id = Some(value as u32);
        } else {
            self.record_debug(field, &value);
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        } else {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        self.record_debug(field, &value);
    }

    fn record_i128(&mut self, field: &tracing::field::Field, value: i128) {
        self.record_debug(field, &value);
    }

    fn record_u128(&mut self, field: &tracing::field::Field, value: u128) {
        self.record_debug(field, &value);
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        self.record_debug(field, &value);
    }

    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.record_debug(field, &value);
    }
}