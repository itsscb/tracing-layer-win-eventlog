use std::collections::HashMap;
use std::ffi::CString;
use tracing::field::Visit;
use tracing::{Level, Subscriber};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::Layer;
use winapi::shared::minwindef::DWORD;
use winapi::um::winbase::{DeregisterEventSource, RegisterEventSourceA, ReportEventA};
use winapi::um::winnt::{EVENTLOG_ERROR_TYPE, EVENTLOG_INFORMATION_TYPE, EVENTLOG_WARNING_TYPE};

#[allow(clippy::manual_c_str_literals)]
pub fn write_to_event_log(event_id: u32, level: Level, message: &str, log_name: &str) {
    let event_source = unsafe {
        RegisterEventSourceA(
            std::ptr::null(),
            format!("{log_name}\0").as_ptr().cast::<i8>(),
        )
    };

    if event_source.is_null() {
        eprintln!("Failed to register event source");
        return;
    }

    let event_type = match level {
        Level::ERROR => EVENTLOG_ERROR_TYPE,
        Level::WARN => EVENTLOG_WARNING_TYPE,
        Level::INFO | Level::DEBUG | Level::TRACE => EVENTLOG_INFORMATION_TYPE,
    };

    let Ok(message_cstr) = CString::new(message) else {
        eprintln!("failed to create CString from message: {message}");
        return;
    };

    let result = unsafe {
        ReportEventA(
            event_source,
            event_type,
            0,
            event_id as DWORD,
            std::ptr::null_mut(),
            1,
            0,
            &mut message_cstr.as_ptr(),
            std::ptr::null_mut(),
        )
    };

    if result == 0 {
        eprintln!("Failed to write to event log");
    }

    unsafe {
        DeregisterEventSource(event_source);
    }
}

pub struct EventLogLayer {
    log_name: String,
}

impl EventLogLayer {
    #[must_use]
    pub const fn new(log_name: String) -> Self {
        Self { log_name }
    }
}
impl<S> Layer<S> for EventLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        let metadata = event.metadata();

        let mut visitor = EventVisitor {
            id: None,
            message: None,
            parents: None,
            log_level: *metadata.level(),
            log_name: &self.log_name,
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
struct EventVisitor<'a> {
    id: Option<u32>,
    log_level: Level,
    message: Option<String>,
    parents: Option<String>,
    fields: HashMap<String, String>,
    log_name: &'a str,
}

impl<'a> EventVisitor<'a> {
    fn log(&self) {
        let id: u32 = self.id.unwrap_or(match self.log_level {
            Level::TRACE => 0,
            Level::DEBUG => 1,
            Level::INFO => 2,
            Level::WARN => 3,
            Level::ERROR => 4,
        });

        let mut msg = format!("ID: {id}\n\n");

        if let Some(m) = &self.parents {
            msg.push_str(&format!("source: {m}\n"));
        }
        if let Some(m) = &self.message {
            msg.push_str(&format!("message: {m}\n"));
        }

        self.fields.iter().for_each(|i| {
            msg.push_str(&format!("{}: {:?}\n", i.0, i.1.replace(r"\\", r"\")));
        });

        write_to_event_log(id, self.log_level, &msg, self.log_name);
    }
}

impl<'a> Visit for EventVisitor<'a> {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name().to_lowercase() == "id" {
            self.id = Some(value as u32);
        } else {
            self.fields
                .insert(field.name().to_string(), format!("{value}"));
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name().to_lowercase() == "id" {
            self.id = Some(value as u32);
        } else {
            self.fields
                .insert(field.name().to_string(), format!("{value:?}"));
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name().to_lowercase() == "id" {
            self.id = Some(format!("{value:?}").parse().unwrap_or(0));
        } else if field.name() == "message" {
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
