# pprof

```rust

let guard = pprof::ProfilerGuard::new(100).unwrap();

// Some codes

match guard.report().build() {
    Ok(report) => {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("report: {}", &report);
    }
    Err(_) => {}
};

```
