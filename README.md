# tracing-layer-win-eventlog
Layer for the *tracing_subscriber* to write to the Windows EventLog

## Usage
```rust
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

fn main() {
    println!("Hello, world!");

    tracing_subscriber::registry()
        .with(tracing_layer_win_eventlog::EventLogLayer)
        .init();

    tracing::info!(id = 40, "hello world!");
}

```
