// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

use crate::Result;

fn fold<W>(report: &pprof::Report, with_thread_name: bool, mut writer: W) -> Result<()>
where
    W: std::io::Write,
{
    for (key, value) in report.data.iter() {
        if with_thread_name {
            if !key.thread_name.is_empty() {
                write!(writer, "{};", key.thread_name)?;
            } else {
                write!(writer, "{:?};", key.thread_id)?;
            }
        }

        let last_frame = key.frames.len() - 1;
        for (index, frame) in key.frames.iter().rev().enumerate() {
            let last_symbol = frame.len() - 1;
            for (index, symbol) in frame.iter().rev().enumerate() {
                if index == last_symbol {
                    write!(writer, "{}", symbol)?;
                } else {
                    write!(writer, "{};", symbol)?;
                }
            }

            if index != last_frame {
                write!(writer, ";")?;
            }
        }

        writeln!(writer, " {}", value)?;
    }

    Ok(())
}

pub async fn pyroscope_ingest<S: AsRef<str>, N: AsRef<str>>(
    report: &mut pprof::Report,
    url: S,
    application_name: N,
) -> Result<()> {
    let mut buffer = Vec::new();

    fold(report, true, &mut buffer)?;

    let client = reqwest::Client::new();
    // TODO: handle the error of this request

    let start: u64 = report
        .timing
        .start_time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let s_start = start - start.checked_rem(10).unwrap();
    // This assumes that the interval between start and until doesn't
    // exceed 10s
    let s_until = s_start + 10;

    client
        .post(format!("{}/ingest", url.as_ref()))
        .header("Content-Type", "application/json")
        .query(&[
            ("name", application_name.as_ref()),
            ("from", &format!("{}", s_start)),
            ("until", &format!("{}", s_until)),
            ("format", "folded"),
            ("sampleRate", &format!("{}", report.sample_rate)),
            ("spyName", "pprof-rs"),
        ])
        .body(buffer)
        .send()
        .await?;

    Ok(())
}