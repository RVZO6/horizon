//! Checks the platform layer in a browser (web/probe.html): threads, scoped threads, channels,
//! mutex contention on the main thread, sleep, clocks and the in-memory file system.
#[cfg(target_arch = "wasm32")]
use horizon_web as _;

#[cfg(target_arch = "wasm32")]
mod probe {
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant, SystemTime};
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub fn web_probe() -> Result<String, JsError> {
        let mut report = Vec::new();
        let started = Instant::now();
        report.push(format!("now: {:?}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?));
        report.push(format!("parallelism: {:?}", std::thread::available_parallelism()?));
        let (tx, rx) = mpsc::channel();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let tx = tx.clone();
                std::thread::Builder::new()
                    .name(format!("probe-{i}"))
                    .spawn(move || {
                        std::thread::sleep(Duration::from_millis(50));
                        tx.send((i, std::thread::current().name().map(str::to_owned))).unwrap();
                        i * 10
                    })
                    .unwrap()
            })
            .collect();
        drop(tx);
        let mut got: Vec<_> = rx.iter().collect();
        got.sort();
        report.push(format!("channel: {got:?}"));
        let joined: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        report.push(format!("joined: {joined:?}"));
        // The main thread contends with workers for one lock.
        let counter = Arc::new(Mutex::new(0u64));
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let counter = counter.clone();
                scope.spawn(move || {
                    for _ in 0..20000 {
                        *counter.lock().unwrap() += 1;
                    }
                });
            }
            for _ in 0..20000 {
                *counter.lock().unwrap() += 1;
            }
        });
        report.push(format!("contended counter: {} (want 100000)", counter.lock().unwrap()));
        std::fs::create_dir_all("probe/dir")?;
        std::fs::write("probe/dir/a.txt", b"hello")?;
        std::fs::rename("probe/dir", "probe/moved")?;
        report.push(format!("fs: {:?} cwd {:?}", std::fs::read_to_string("probe/moved/a.txt")?, std::env::current_dir()?));
        let lazy = std::fs::read("/res/horizon-resources.json");
        report.push(format!("lazy: {:?}", lazy.map(|b| String::from_utf8_lossy(&b).chars().take(60).collect::<String>())));
        let names: Vec<_> = std::fs::read_dir("/res")?.take(5).map(|e| e.map(|e| e.file_name())).collect();
        report.push(format!("read_dir /res: {names:?}"));
        report.push(format!("elapsed: {:?}", started.elapsed()));
        eprintln!("probe stderr line");
        Ok(report.join("\n"))
    }
}

fn main() {}
