mod watchdog;
use std::time::Duration;

use rusty_v8 as v8;
use watchdog::{WatchOptions, Watchdog};

fn main() {
    let platform = v8::new_default_platform(0, false).make_shared();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let handle_scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(handle_scope);
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let code = v8::String::new(scope, "for(;;);").unwrap();
    let script = v8::Script::compile(scope, code, None).unwrap();

    let watchdog = Watchdog::new();
    watchdog.watch(
        &scope.thread_safe_handle(),
        WatchOptions {
            cpu_timeout: Duration::from_millis(20),
        },
    );

    assert_eq!(script.run(scope), None);

    println!("successfully terminated from the infinite loop");
}
