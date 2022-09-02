use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use fixtures::RaftRouter;
use maplit::btreeset;
use openraft::Config;
use openraft::RaftStorage;
use tokio::time::sleep;

#[macro_use]
mod fixtures;

/// Logs should be deleted by raft after applying them, on leader and non-voter.
///
/// - assert logs are deleted on leader after applying them.
/// - assert logs are deleted on replication target after installing a snapshot.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clean_applied_logs() -> Result<()> {
    let (_log_guard, ut_span) = init_ut!();
    let _ent = ut_span.enter();

    // Setup test dependencies.
    let config = Arc::new(
        Config {
            max_applied_log_to_keep: 2,
            ..Default::default()
        }
        .validate()?,
    );
    let router = Arc::new(RaftRouter::new(config.clone()));

    let mut n_logs = router.new_nodes_from_single(btreeset! {0}, btreeset! {1}).await?;

    let count = (10 - n_logs) as usize;
    for idx in 0..count {
        router.client_request(0, "0", idx as u64).await;
        // raft commit at once with a single leader cluster.
        // If we send too fast, logs are removed before forwarding to non-voter.
        // Then it triggers snapshot replication, which is not expected.
        sleep(Duration::from_millis(50)).await;
    }
    n_logs = 10;

    router.wait_for_log(&btreeset! {0,1}, n_logs, timeout(), "write upto 10 logs").await?;

    tracing::info!("--- logs before max_applied_log_to_keep should be cleaned");
    {
        for node_id in 0..1 {
            let sto = router.get_storage_handle(&node_id).await?;
            let logs = sto.get_log_entries(..).await?;
            assert_eq!(2, logs.len(), "node {} should have only {} logs", node_id, 2);
        }
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(5000))
}