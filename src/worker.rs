use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use crate::api::{fetch_gateway, fetch_latest, fetch_stats, fetch_tasks, http_client};
use crate::config::Config;
use crate::model::Update;

pub fn worker(config: Config, tx: Sender<Update>, refresh: Arc<AtomicBool>) {
    let client = http_client(&config);
    let mut last_run: Option<Instant> = None;
    let mut last_version_check: Option<Instant> = None;
    loop {
        let due = refresh.swap(false, Ordering::SeqCst)
            || last_run.is_none_or(|t| t.elapsed() >= config.interval);
        if due {
            // One message per source so each card fills in as soon as its data is there.
            let _ = tx.send(Update {
                gateway: Some(fetch_gateway(&client, &config).map(|r| r.servers)),
                ..Default::default()
            });
            let _ = tx.send(Update {
                stats: Some(fetch_stats(&client, &config).map(|r| r.data)),
                ..Default::default()
            });
            let _ = tx.send(Update {
                tasks: Some(fetch_tasks(&client, &config)),
                ..Default::default()
            });
            last_run = Some(Instant::now());
        }
        // After the data, so a slow GitHub check never holds up the panel.
        if last_version_check.is_none_or(|t| t.elapsed() >= Duration::from_secs(3600)) {
            let latest = fetch_latest(&client, &config);
            last_version_check = Some(Instant::now());
            let _ = tx.send(Update {
                latest_v2: Some(latest),
                ..Default::default()
            });
        }
        thread::sleep(Duration::from_millis(500));
    }
}
