// SPDX-License-Identifier: Apache-2.0
use std::time::Duration;

use tokio::time::sleep;

use super::{PromptClock, Stage, Timeouts, within};
use crate::SshError;

const BUDGET: Duration = Duration::from_secs(1);

async fn prompt_then_work(
    clock: PromptClock,
    prompt: Duration,
    work: Duration,
) -> Result<(), SshError> {
    {
        let _pause = clock.pause();
        sleep(prompt).await;
    }
    sleep(work).await;
    Ok(())
}

#[test]
fn defaults_match_reliability_doc() {
    let timeouts = Timeouts::default();
    assert_eq!(timeouts.dns, Duration::from_secs(5));
    assert_eq!(timeouts.tcp_connect, Duration::from_secs(10));
    assert_eq!(timeouts.handshake, Duration::from_secs(20));
    assert_eq!(timeouts.keepalive_interval, Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn stalled_stage_times_out_with_its_name() {
    let clock = PromptClock::default();
    let result = within(
        Stage::Handshake,
        BUDGET,
        &clock,
        std::future::pending::<Result<(), SshError>>(),
    )
    .await;
    assert!(matches!(
        result,
        Err(SshError::Timeout { stage: Stage::Handshake, after }) if after == BUDGET
    ));
}

#[tokio::test(start_paused = true)]
async fn prompt_time_is_not_charged_to_the_stage() {
    let clock = PromptClock::default();
    let work = prompt_then_work(
        clock.clone(),
        Duration::from_secs(60),
        Duration::from_millis(900),
    );
    assert!(within(Stage::Handshake, BUDGET, &clock, work).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn budget_still_applies_after_a_prompt() {
    let clock = PromptClock::default();
    let work = prompt_then_work(
        clock.clone(),
        Duration::from_secs(60),
        Duration::from_secs(2),
    );
    let result = within(Stage::Handshake, BUDGET, &clock, work).await;
    assert!(matches!(result, Err(SshError::Timeout { .. })));
}
