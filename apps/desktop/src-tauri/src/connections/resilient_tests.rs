// SPDX-License-Identifier: Apache-2.0
//! A cloud token opens one connection, then every reconnect fetches a new one.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn counting(fetches: &Arc<AtomicUsize>) -> FreshPassword {
    let fetches = Arc::clone(fetches);
    Arc::new(move || {
        let count = fetches.fetch_add(1, Ordering::SeqCst) + 1;
        Box::pin(async move { Ok(SecretString::from(format!("fresh-{count}").as_str())) })
    })
}

#[tokio::test]
async fn the_opening_token_is_used_once_and_never_replayed() -> Result<(), DriverError> {
    let fetches = Arc::new(AtomicUsize::new(0));
    let password = OneTimePassword::new(Some("opening".into()), counting(&fetches));
    assert_eq!(password.next().await?.expose(), "opening");
    assert_eq!(fetches.load(Ordering::SeqCst), 0);
    assert_eq!(password.next().await?.expose(), "fresh-1");
    assert_eq!(password.next().await?.expose(), "fresh-2");
    assert_eq!(fetches.load(Ordering::SeqCst), 2);
    Ok(())
}
