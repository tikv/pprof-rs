# rsperftools

```rust

rsperftools::PROFILER.write().unwrap().start(100).unwrap();

// Some codes

match rsperftools::PROFILER.read().unwrap().report().build() {
    Ok(report) => {
        let file = File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();

        println!("report: {}", &report);
    }
    Err(_) => {}
};

```
