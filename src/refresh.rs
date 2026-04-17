use std::{sync::Arc, time::Duration};

use rocksdb::DB;
use tokio_util::sync::CancellationToken;

pub fn spawn(db: Arc<DB>, interval: Duration, shutdown: CancellationToken) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // Don't fire the first tick immediately — the DB was just opened and
        // therefore is already current.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await;
        tracing::info!(
            ?interval,
            "secondary refresh task started (try_catch_up_with_primary)"
        );
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("secondary refresh task shutting down");
                    return;
                }
                _ = ticker.tick() => {
                    if let Err(e) = db.try_catch_up_with_primary() {
                        tracing::warn!(error = %e, "try_catch_up_with_primary failed");
                    }
                }
            }
        }
    });
}
