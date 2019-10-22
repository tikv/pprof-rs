# rsperftools

```rust

rsperftools::PROFILER.lock().unwrap().start(100).unwrap();

// Some codes

match rsperftools::PROFILER.lock().unwrap().report() {
    Ok(report) => {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("report: {}", &report);
    }
    Err(_) => {}
};

```