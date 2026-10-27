//! Checks the platform layer in a browser (web/probe.html): threads, scoped threads, channels,
//! mutex contention on the main thread, sleep, clocks and the in-memory file system.
#[cfg(target_arch = "wasm32")]
use horizon_web as _;

#[cfg(target_arch = "wasm32")]
mod probe {
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant, SystemTime};
    use wasm_bindgen::prelude::*;

    /// `File` as std's unix backend behaves: each check's name, or "ok".
    fn file_semantics() -> Result<String, std::io::Error> {
        use std::fs::{File, OpenOptions};
        use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
        std::fs::write("semantics.txt", b"0123456789")?;
        let mut failed = Vec::new();
        // A duplicate shares the cursor.
        let mut a = File::open("semantics.txt")?;
        let mut b = a.try_clone()?;
        let mut two = [0u8; 2];
        a.read_exact(&mut two)?;
        b.read_exact(&mut two)?;
        if &two != b"23" {
            failed.push("duplicate cursor");
        }
        // A read past the end reads nothing and keeps the cursor.
        a.seek(SeekFrom::Start(20))?;
        if a.read(&mut two)? != 0 || a.stream_position()? != 20 {
            failed.push("read past the end");
        }
        // set_len needs write access.
        if File::open("semantics.txt")?.set_len(2).is_ok() {
            failed.push("set_len read-only");
        }
        // The open options std's unix backend refuses.
        let refused = |o: &mut OpenOptions| {
            o.open("semantics.txt").err().map(|e| e.kind()) == Some(ErrorKind::InvalidInput)
        };
        if !refused(OpenOptions::new().create(true)) {
            failed.push("create without write");
        }
        if !refused(OpenOptions::new().append(true).truncate(true)) {
            failed.push("append with truncate");
        }
        // Writes past the end fill with zeros; the length follows.
        let mut w = OpenOptions::new().write(true).open("semantics.txt")?;
        w.seek(SeekFrom::Start(12))?;
        w.write_all(b"x")?;
        if std::fs::metadata("semantics.txt")?.len() != 13
            || std::fs::read("semantics.txt")?[10..] != *b"\0\0x"
        {
            failed.push("write past the end");
        }
        Ok(if failed.is_empty() {
            "ok".into()
        } else {
            failed.join(", ")
        })
    }

    /// The page's thread joins a thread that panics (and so traps, with panic=abort): the join
    /// must end, with a trap, instead of spinning forever.
    #[wasm_bindgen]
    pub fn web_probe_failing_thread() -> String {
        let thread = std::thread::spawn(|| panic!("a probe thread fails on purpose"));
        let joined = thread.join();
        format!("the join returned: {joined:?}")
    }

    #[wasm_bindgen]
    pub fn web_probe() -> Result<String, JsError> {
        let mut report = Vec::new();
        let started = Instant::now();
        report.push(format!(
            "now: {:?}",
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?
        ));
        report.push(format!(
            "parallelism: {:?}",
            std::thread::available_parallelism()?
        ));
        let (tx, rx) = mpsc::channel();
        let handles: Vec<_> = (0..4)
            .map(|i| {
                let tx = tx.clone();
                std::thread::Builder::new()
                    .name(format!("probe-{i}"))
                    .spawn(move || {
                        std::thread::sleep(Duration::from_millis(50));
                        tx.send((i, std::thread::current().name().map(str::to_owned)))
                            .unwrap();
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
        report.push(format!(
            "contended counter: {} (want 100000)",
            counter.lock().unwrap()
        ));
        std::fs::create_dir_all("probe/dir")?;
        std::fs::write("probe/dir/a.txt", b"hello")?;
        std::fs::rename("probe/dir", "probe/moved")?;
        report.push(format!(
            "fs: {:?} cwd {:?}",
            std::fs::read_to_string("probe/moved/a.txt")?,
            std::env::current_dir()?
        ));
        let lazy = std::fs::read("/res/horizon-resources.json");
        report.push(format!(
            "lazy: {:?}",
            lazy.map(|b| String::from_utf8_lossy(&b)
                .chars()
                .take(60)
                .collect::<String>())
        ));
        let names: Vec<_> = std::fs::read_dir("/res")?
            .take(5)
            .map(|e| e.map(|e| e.file_name()))
            .collect();
        report.push(format!("read_dir /res: {names:?}"));
        report.push(format!("file semantics: {:?}", file_semantics()));
        report.push(format!("elapsed: {:?}", started.elapsed()));
        eprintln!("probe stderr line");
        Ok(report.join("\n"))
    }
}

fn main() {}
