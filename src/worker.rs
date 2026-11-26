use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::api::{fetch_gateway, fetch_latest, fetch_stats, http_client};
use crate::config::Config;
use crate::model::{Release, Snapshot};

pub fn worker(config: Config, tx: Sender<Snapshot>, refresh: Arc<AtomicBool>) {
    let client = http_client(&config);
    let mut last_run: Option<Instant> = None;
    let mut latest_v2: Result<Option<Release>, String> = Ok(None);
    let mut last_version_check: Option<Instant> = None;
    loop {
        if last_version_check.is_none_or(|t| t.elapsed() >= Duration::from_secs(3600)) {
            latest_v2 = fetch_latest(&client, &config);
            last_version_check = Some(Instant::now());
        }
        let due = refresh.swap(false, Ordering::SeqCst)
            || last_run.is_none_or(|t| t.elapsed() >= config.interval);
        if due {
            let gateway = fetch_gateway(&client, &config).map(|r| r.servers);
            let stats = fetch_stats(&client, &config).map(|r| r.data);
            let _ = tx.send(Snapshot {
                gateway,
                stats,
                latest_v2: latest_v2.clone(),
                at: Instant::now(),
            });
            last_run = Some(Instant::now());
        }
        thread::sleep(Duration::from_millis(500));
    }
}
