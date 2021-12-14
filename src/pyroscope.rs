// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! this mod could help you to upload profiler data to the pyroscope

use std::collections::HashMap;

use crate::Result;

use tokio::sync::mpsc;

pub struct PyroscopeAgent {
    stopper: mpsc::Sender<()>,

    handler: tokio::task::JoinHandle<Result<()>>,
}

impl PyroscopeAgent {
    pub async fn new(
        url: String,
        frequency: libc::c_int,
        application_name: String,
        tags: Option<HashMap<String, String>>,
    ) -> Self {
        let application_name = merge_tags_with_app_name(application_name, tags);
        let (stopper, mut stop_signal) = mpsc::channel::<()>(1);

        // Since Pyroscope only allow 10s intervals, it might not be necessary
        // to make this customizable at this point
        let upload_interval = std::time::Duration::from_secs(10);
        let mut interval = tokio::time::interval(upload_interval);

        let handler = tokio::spawn(async move {
            loop {
                let guard = super::ProfilerGuard::new(frequency).unwrap();

                tokio::select! {
                    _ = interval.tick() => {
                        guard.report().build()?.pyroscope_ingest(&url, &application_name).await?;
                    }
                    _ = stop_signal.recv() => {
                        guard.report().build()?.pyroscope_ingest(&url, &application_name).await?;

                        break Ok(())
                    }
                }
            }
        });

        Self { stopper, handler }
    }

    pub async fn stop(self) -> Result<()> {
        self.stopper.send(()).await.unwrap();

        self.handler.await.unwrap()?;

        Ok(())
    }
}

fn merge_tags_with_app_name(
    application_name: String,
    tags: Option<HashMap<String, String>>,
) -> String {
    format!(
        "{}{}",
        application_name,
        tags.map(|tags| tags
            .into_iter()
            .filter(|(k, _)| k != "__name__")
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<String>>())
            .map(|mut tags| {
                tags.sort();

                format!("{{{}}}", tags.join(","))
            })
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::pyroscope::merge_tags_with_app_name;

    #[test]
    fn merge_tags_with_app_name_with_tags() {
        let mut tags = HashMap::new();
        tags.insert("env".to_string(), "staging".to_string());
        tags.insert("region".to_string(), "us-west-1".to_string());
        tags.insert("__name__".to_string(), "reserved".to_string());
        assert_eq!(
            merge_tags_with_app_name("my.awesome.app.cpu".to_string(), Some(tags)),
            "my.awesome.app.cpu{env=staging,region=us-west-1}".to_string()
        )
    }

    #[test]
    fn merge_tags_with_app_name_without_tags() {
        assert_eq!(
            merge_tags_with_app_name("my.awesome.app.cpu".to_string(), None),
            "my.awesome.app.cpu".to_string()
        )
    }
}
