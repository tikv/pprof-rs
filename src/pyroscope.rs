// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! this mod could help you to upload profiler data to the pyroscope

use crate::Result;

use tokio::sync::mpsc;

pub struct PyroscopeAgent {
    stopper: mpsc::Sender<()>,

    handler: tokio::task::JoinHandle<Result<()>>,
}

impl PyroscopeAgent {
    pub async fn new(
        url: String,
        upload_interval: std::time::Duration,
        frequency: libc::c_int,
        application_name: String,
    ) -> Self {
        let (stopper, mut stop_signal) = mpsc::channel::<()>(1);
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
