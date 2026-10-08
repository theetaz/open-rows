// SPDX-License-Identifier: Apache-2.0
use std::sync::{Arc, Mutex, PoisonError};

use openrows_net::{
    Challenge, ChallengePrompt, FirstUseDecision, HostKeyPolicy, KeyboardInteractivePrompter,
    UnknownHost,
};

use super::*;

/// Records every event; `visible` false simulates a missing webview.
struct Recorder {
    events: Arc<Mutex<Vec<SshPromptChanged>>>,
    visible: bool,
}

impl PromptSink for Recorder {
    fn send(&self, event: SshPromptChanged) -> bool {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(event);
        self.visible
    }
}

fn new_broker(visible: bool) -> (Arc<PromptBroker>, Arc<Mutex<Vec<SshPromptChanged>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = Recorder {
        events: Arc::clone(&events),
        visible,
    };
    (Arc::new(PromptBroker::new(Box::new(sink))), events)
}

fn snapshot(events: &Mutex<Vec<SshPromptChanged>>) -> Vec<SshPromptChanged> {
    events
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
}

/// Wait until a prompt is open, then answer it with `answer`.
async fn answer_when_open(
    broker: &PromptBroker,
    events: &Mutex<Vec<SshPromptChanged>>,
    answer: SshPromptAnswer,
) -> SshPromptDto {
    loop {
        let open = snapshot(events)
            .into_iter()
            .find_map(|event| event.prompt.map(|prompt| (event.id, prompt)));
        if let Some((id, prompt)) = open {
            assert!(broker.answer(&id, answer).is_ok());
            return prompt;
        }
        tokio::task::yield_now().await;
    }
}

fn unknown_host() -> UnknownHost {
    UnknownHost {
        host: "bastion.example.com".to_owned(),
        port: 22,
        algorithm: "ssh-ed25519".to_owned(),
        fingerprint: "SHA256:synthetic-fingerprint".to_owned(),
        other_algorithms: Vec::new(),
    }
}

#[test]
fn trusting_a_host_key_remembers_it_and_closes_the_sheet() {
    let (broker, events) = new_broker(true);
    let policy = UiHostKeyPolicy(Arc::clone(&broker));
    let host = unknown_host();
    let (decision, shown) = tauri::async_runtime::block_on(async {
        tokio::join!(
            policy.on_unknown_host(&host),
            answer_when_open(&broker, &events, SshPromptAnswer::Trust)
        )
    });
    assert_eq!(decision, FirstUseDecision::AcceptAndRemember);
    assert!(matches!(
        shown,
        SshPromptDto::HostKey { ref fingerprint, port: 22, .. } if fingerprint == "SHA256:synthetic-fingerprint"
    ));
    let recorded = snapshot(&events);
    assert_eq!(recorded.len(), 2);
    assert_eq!(
        recorded[1].prompt, None,
        "the sheet closes after the answer"
    );
}

#[test]
fn cancelling_or_an_invisible_webview_rejects_the_key() {
    let (broker, events) = new_broker(true);
    let policy = UiHostKeyPolicy(Arc::clone(&broker));
    let host = unknown_host();
    let (decision, _) = tauri::async_runtime::block_on(async {
        tokio::join!(
            policy.on_unknown_host(&host),
            answer_when_open(&broker, &events, SshPromptAnswer::Cancel)
        )
    });
    assert_eq!(decision, FirstUseDecision::Reject);
    let (hidden, _) = new_broker(false);
    let policy = UiHostKeyPolicy(hidden);
    let decision = tauri::async_runtime::block_on(policy.on_unknown_host(&host));
    assert_eq!(decision, FirstUseDecision::Reject);
}

fn otp_challenge() -> Challenge {
    Challenge {
        host: "bastion.example.com".to_owned(),
        user: "ops".to_owned(),
        name: String::new(),
        instructions: "Two-factor authentication".to_owned(),
        prompts: vec![ChallengePrompt {
            text: "Verification code: ".to_owned(),
            echo: false,
        }],
    }
}

#[test]
fn challenges_need_one_answer_per_prompt() {
    let (broker, events) = new_broker(true);
    let prompter = UiPrompter(Arc::clone(&broker));
    let challenge = otp_challenge();
    let answer = SshPromptAnswer::Answers {
        values: vec!["123456".to_owned()],
    };
    let (answers, shown) = tauri::async_runtime::block_on(async {
        tokio::join!(
            prompter.respond(&challenge),
            answer_when_open(&broker, &events, answer)
        )
    });
    let answers = answers.unwrap_or_default();
    assert_eq!(answers.len(), 1);
    assert_eq!(answers[0].expose(), "123456");
    assert!(matches!(shown, SshPromptDto::Challenge { ref prompts, .. } if !prompts[0].echo));

    let (broker, events) = new_broker(true);
    let prompter = UiPrompter(Arc::clone(&broker));
    let too_many = SshPromptAnswer::Answers {
        values: vec!["1".to_owned(), "2".to_owned()],
    };
    let (answers, _) = tauri::async_runtime::block_on(async {
        tokio::join!(
            prompter.respond(&challenge),
            answer_when_open(&broker, &events, too_many)
        )
    });
    assert!(answers.is_err());
}

#[test]
fn unknown_or_abandoned_prompts_are_closed() {
    let (broker, events) = new_broker(true);
    let missing = broker.answer("ssh-404", SshPromptAnswer::Cancel);
    assert!(missing.is_err_and(|error| error.kind == IpcErrorKind::NotFound));
    let host = unknown_host();
    let policy = UiHostKeyPolicy(Arc::clone(&broker));
    tauri::async_runtime::block_on(async {
        let pending = policy.on_unknown_host(&host);
        // Dropping the connect abandons the prompt.
        let _ = tokio::time::timeout(Duration::from_millis(5), pending).await;
    });
    let recorded = snapshot(&events);
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[1].prompt, None);
    let id = recorded[0].id.clone();
    assert!(broker.answer(&id, SshPromptAnswer::Trust).is_err());
}

#[test]
fn answers_never_appear_in_debug_output() {
    let answer = SshPromptAnswer::Answers {
        values: vec!["otp-synthetic-123456".to_owned()],
    };
    let text = format!("{answer:?}");
    assert_eq!(text, "Answers([1 redacted])");
}
