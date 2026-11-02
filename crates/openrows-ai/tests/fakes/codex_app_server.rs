// SPDX-License-Identifier: Apache-2.0
//! Test double for `codex app-server`: the JSON-RPC subset the bridge uses.
//!
//! The scenario is read from `$CODEX_HOME/scenario`; launches, requests and
//! the bridge's replies are written to `$CODEX_HOME/*.json` for assertions.
use serde_json::{Value, json};
use std::{
    io::{BufRead as _, Write as _},
    path::PathBuf,
};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let home = PathBuf::from(std::env::var_os("CODEX_HOME").ok_or("no CODEX_HOME")?);
    let scenario = std::fs::read_to_string(home.join("scenario")).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("--version") => {
            let version = std::fs::read_to_string(home.join("version"))
                .unwrap_or_else(|_| "codex-cli 0.166.0".into());
            writeln!(std::io::stdout(), "{}", version.trim())?;
            Ok(())
        }
        Some("linger") => linger(),
        Some("app-server") => Server::new(home, scenario.trim().into(), &args)?.serve(),
        _ => Err("unsupported arguments".into()),
    }
}
/// A grandchild that only ends when its process group is killed.
fn linger() -> Result {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    loop {
        std::thread::park();
    }
}
struct Server {
    home: PathBuf,
    scenario: String,
    signed_in: bool,
    out: std::io::Stdout,
    input: std::io::Lines<std::io::StdinLock<'static>>,
    lingering: Option<std::process::Child>,
}
impl Server {
    fn new(home: PathBuf, scenario: String, args: &[String]) -> Result<Self> {
        let mut environment: Vec<String> = std::env::vars_os()
            .map(|(name, _)| name.to_string_lossy().into_owned())
            .collect();
        environment.sort();
        let launch = json!({
            "args": args, "environment": environment,
            "home": std::env::var("HOME")?, "cwd": std::env::current_dir()?,
        });
        std::fs::write(home.join("launch.json"), launch.to_string())?;
        Ok(Self {
            signed_in: !matches!(scenario.as_str(), "signed-out" | "bad-url"),
            home,
            scenario,
            out: std::io::stdout(),
            input: std::io::stdin().lock().lines(),
            lingering: None,
        })
    }
    fn serve(mut self) -> Result {
        while let Some(line) = self.input.next() {
            let message: Value = serde_json::from_str(&line?)?;
            let Some(method) = message["method"].as_str().map(str::to_owned) else {
                continue;
            };
            if message.get("id").is_some() {
                self.request(&method, &message["id"], &message["params"])?;
            }
        }
        Ok(())
    }
    fn send(&mut self, value: &Value) -> Result {
        writeln!(self.out, "{value}")?;
        Ok(self.out.flush()?)
    }
    fn record(&self, name: &str, value: &Value) -> Result {
        Ok(std::fs::write(
            self.home.join(format!("{name}.json")),
            value.to_string(),
        )?)
    }
    fn request(&mut self, method: &str, id: &Value, params: &Value) -> Result {
        let result = match method {
            "initialize" => {
                self.record("initialize", params)?;
                json!({"userAgent": "openrows/0.166.0 (fake)", "codexHome": self.home, "platformFamily": "unix", "platformOs": "macos"})
            }
            "account/read" => self.account(),
            "model/list" => json!({"data": [
                {"id": "a", "model": "gpt-fake", "displayName": "GPT Fake", "hidden": false, "inputModalities": ["text", "image"]},
                {"id": "b", "model": "gpt-hidden", "displayName": "Hidden", "hidden": true, "inputModalities": ["text"]},
            ], "nextCursor": null}),
            "account/login/start" => return self.login(id),
            "thread/start" => {
                self.record("thread", params)?;
                json!({"thread": {"id": "thread-1"}, "model": params["model"]})
            }
            "turn/start" => {
                self.record("turn", params)?;
                self.send(&json!({"id": id, "result": {"turn": {"id": "turn-1", "status": "inProgress", "items": [], "error": null}}}))?;
                return self.play();
            }
            _ => {
                return self.send(&json!({"id": id, "error": {"code": -32600, "message": "Invalid request: unknown variant"}}));
            }
        };
        self.send(&json!({"id": id, "result": result}))
    }
    fn account(&self) -> Value {
        if self.signed_in {
            json!({"account": {"type": "chatgpt", "email": "person@example.com", "planType": "plus"}, "requiresOpenaiAuth": true})
        } else {
            json!({"account": null, "requiresOpenaiAuth": true})
        }
    }
    fn login(&mut self, id: &Value) -> Result {
        let url = if self.scenario == "bad-url" {
            "http://evil.example/login"
        } else {
            "https://auth.openai.com/oauth/authorize?state=fake"
        };
        self.send(
            &json!({"id": id, "result": {"type": "chatgpt", "loginId": "login-1", "authUrl": url}}),
        )?;
        self.signed_in = true;
        self.send(&json!({"method": "account/login/completed", "params": {"loginId": "login-1", "success": true, "error": null}}))
    }
    fn delta(&mut self, text: &str) -> Result {
        self.send(&json!({"method": "item/agentMessage/delta", "params": {"threadId": "thread-1", "turnId": "turn-1", "itemId": "item", "delta": text}}))
    }
    fn complete(&mut self, status: &str, info: &Value) -> Result {
        self.send(&json!({"method": "turn/completed", "params": {"threadId": "thread-1", "turn": {"id": "turn-1", "status": status, "items": [], "error": {"message": "detail", "codexErrorInfo": info}}}}))
    }
    /// Ask the bridge and record its reply under `name`.
    fn ask(&mut self, name: &str, method: &str, params: &Value) -> Result {
        self.send(&json!({"id": name, "method": method, "params": params}))?;
        let line = self.input.next().ok_or("closed")??;
        self.record(name, &serde_json::from_str(&line)?)
    }
    fn play(&mut self) -> Result {
        match self.scenario.as_str() {
            "tool" => self.tool(),
            "approval" => self.approvals(),
            "crash" => {
                self.delta("partial")?;
                std::process::exit(3)
            }
            "hang" => self.hang(),
            "flood" => self.delta(&"x".repeat(2 << 20)),
            "failed" => self.complete("failed", &json!("usageLimitExceeded")),
            _ => {
                self.delta("Hello ")?;
                self.delta("world")?;
                self.complete("completed", &Value::Null)
            }
        }
    }
    fn tool(&mut self) -> Result {
        self.delta("Checking. ")?;
        let call = json!({"threadId": "thread-1", "turnId": "turn-1", "callId": "call-1", "namespace": null, "tool": "run_read_query", "arguments": {"sql": "SELECT id FROM public.people"}});
        self.ask("tool-result", "item/tool/call", &call)?;
        self.delta("Done.")?;
        self.complete("completed", &Value::Null)
    }
    fn approvals(&mut self) -> Result {
        let scope = json!({"threadId": "thread-1", "turnId": "turn-1", "itemId": "item"});
        self.ask(
            "command-approval",
            "item/commandExecution/requestApproval",
            &scope,
        )?;
        self.ask("file-approval", "item/fileChange/requestApproval", &scope)?;
        self.ask("permissions", "item/permissions/requestApproval", &scope)?;
        self.ask("elicitation", "mcpServer/elicitation/request", &scope)?;
        self.ask("user-input", "item/tool/requestUserInput", &scope)?;
        self.delta("Declined.")?;
        self.complete("completed", &Value::Null)
    }
    /// Start a grandchild in the same process group, report both PIDs, then stall.
    fn hang(&mut self) -> Result {
        let child = std::process::Command::new(std::env::current_exe()?)
            .arg("linger")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        self.record("pids", &json!([std::process::id(), child.id()]))?;
        self.lingering = Some(child);
        self.delta("partial")?;
        loop {
            std::thread::park();
        }
    }
}
