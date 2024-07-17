# tracing-layer-win-eventlog
Layer for the *tracing_subscriber* to write to the Windows EventLog

## Usage

If the Windows EventLog does not yet exist, it has to be created first.
```powershell
# PowerShell v5.1 running as Administrator
New-EventLog -LogName Application -Source hello_world

```

```rust
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

fn main() {
    let eventlog = tracing_layer_win_eventlog::EventLogLayer::new("hello_world".to_owned());

    tracing_subscriber::registry()
        .with(eventlog)
        .init();

    tracing::info!(id = 40, "hello world!");
}

```

The `id` is optional and used as the Windows EventID and has to be `unsigned`. If no `id` is given, the `tracing::Level` will be used as the EventID.

The parent spans are listed above the message in the `source` key. If there are multiple parent spans they are concatenated with the `/` separator.

All other objects that are passed are written below the message in a `key: value` pair.

### Example

```rust
#[tracing::instrument]
fn windows() {
    let path = "C:\\Windows";
    tracing::debug!(id=2,?path,"currently in windir");
}
```

The above example will be written to the EventLog as follows:
```
ID: 2

source: windows
message: currently in windir
path: "\"C:\\Windows\""

```
