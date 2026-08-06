//! A fake editor — the client half of the conversation, over either door the server opens.
//!
//! Two doors, one vocabulary. [`Editor::attached`] runs the server in this process over
//! lsp-server's in-memory transport: it is where the per-capability suite lives, because a
//! panic in a handler surfaces as a failed join with a backtrace rather than as a pipe
//! that went quiet. [`Editor::spawned`] runs the installed executable exactly the way an
//! editor runs it — argv `lsp --stdio`, Content-Length framing over real pipes, a real
//! process to reap — which is the only way to reach the half of `serve_stdio` the
//! in-memory door cannot: the framing codec, and the drop-before-join order whose absence
//! deadlocks a clean exit. Both are the real server; neither is a mock of it.
//!
//! The framing is lsp-server's own [`Message::read`]/[`Message::write`], not a
//! hand-rolled parser, so the spawned door proves the bytes an editor would send are the
//! bytes this server accepts rather than proving this harness agrees with itself.
//!
//! Every read is bounded. A server that never answers should be a red CI job and not a
//! six-hour one, so both doors are pumped into a channel that is only ever read with a
//! deadline.

#![allow(dead_code, reason = "each test binary reaches for a different part of the harness")]
#![allow(clippy::expect_used, reason = "a harness that cannot reach the server has failed")]

use std::io::{BufReader, Write as _};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId, ResponseError};
use serde_json::{Value, json};

/// How long any single answer may take. Generous enough that a loaded CI runner never
/// trips it, short enough that a deadlock is a failure rather than a timeout on the job.
const PATIENCE: Duration = Duration::from_secs(30);

/// The command every adapter in `editors/` spells, and the one [`Editor::spawned`] proves.
/// Sharing the words is what makes `editors.rs`'s parity claim a real link between the
/// adapters and a passing protocol test rather than two lists that happen to agree today.
pub(crate) const EXECUTABLE: &str = "zoning";
/// The argv that turns the executable into a language server.
pub(crate) const SERVE: [&str; 2] = ["lsp", "--stdio"];

/// The client half of an LSP session.
pub(crate) struct Editor {
    outbox: Outbox,
    inbox: Receiver<Message>,
    ending: Ending,
    /// Notifications that arrived while waiting for something else.
    overheard: Vec<Notification>,
    next: i32,
}

/// Where a message goes.
enum Outbox {
    /// The in-memory transport's channel, held inside the client's own `Connection`.
    Memory(Connection),
    /// The child process's standard input.
    Pipe(ChildStdin),
}

/// What proves the server stopped cleanly.
enum Ending {
    Thread(JoinHandle<zoning::Result<()>>),
    Process(Child),
}

impl Editor {
    /// The server in this process, over the in-memory transport.
    pub(crate) fn attached() -> Self {
        let (server, client) = Connection::memory();
        let ending = Ending::Thread(thread::spawn(move || zoning::lsp::serve(&server)));
        let source = client.receiver.clone();
        let inbox = pump(move || source.recv().ok());
        Self { outbox: Outbox::Memory(client), inbox, ending, overheard: Vec::new(), next: 0 }
    }

    /// The installed executable, launched the way every adapter in `editors/` launches it.
    pub(crate) fn spawned() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zoning"))
            .args(SERVE)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("the built executable must be runnable");
        let outbox = Outbox::Pipe(child.stdin.take().expect("stdin was piped"));
        let mut source = BufReader::new(child.stdout.take().expect("stdout was piped"));
        let inbox = pump(move || Message::read(&mut source).ok().flatten());
        Self { outbox, inbox, ending: Ending::Process(child), overheard: Vec::new(), next: 0 }
    }

    /// Shake hands, and return what the server said it can do.
    pub(crate) fn hello(&mut self) -> Value {
        let welcome = self
            .ask("initialize", json!({"processId": null, "rootUri": null, "capabilities": {}}))
            .expect("initialize must be answered");
        self.tell("initialized", json!({}));
        welcome
    }

    /// Send a notification. Nothing comes back, by definition.
    pub(crate) fn tell(&mut self, method: &str, params: Value) {
        self.post(Notification::new(method.to_owned(), params).into());
    }

    /// Send a request and wait for its answer, banking any notification that overtakes it.
    pub(crate) fn ask(&mut self, method: &str, params: Value) -> Result<Value, ResponseError> {
        self.next += 1;
        let id = RequestId::from(self.next);
        self.post(Request::new(id.clone(), method.to_owned(), params).into());
        loop {
            match self.next_message() {
                Message::Response(response) if response.id == id => {
                    return response.response_result;
                }
                Message::Notification(notification) => self.overheard.push(notification),
                other => panic!("{method} was answered with {other:?}"),
            }
        }
    }

    /// Open a document, and return the diagnostics the server published for it.
    pub(crate) fn open(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.tell(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri, "languageId": "zoning", "version": 1, "text": text
            }}),
        );
        self.diagnostics(uri)
    }

    /// Replace a document's whole text, and return the diagnostics that follow.
    pub(crate) fn edit(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.tell(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": 2},
                "contentChanges": [{"text": text}]
            }),
        );
        self.diagnostics(uri)
    }

    /// The next set of diagnostics published for `uri`.
    pub(crate) fn diagnostics(&mut self, uri: &str) -> Vec<Value> {
        let published = |notification: &Notification| {
            notification.method == "textDocument/publishDiagnostics"
                && notification.params["uri"] == uri
        };
        let params = if let Some(index) = self.overheard.iter().position(published) {
            self.overheard.remove(index).params
        } else {
            loop {
                match self.next_message() {
                    Message::Notification(notification) if published(&notification) => {
                        break notification.params;
                    }
                    Message::Notification(notification) => self.overheard.push(notification),
                    other => panic!("waiting on diagnostics for {uri}, got {other:?}"),
                }
            }
        };
        params["diagnostics"].as_array().cloned().unwrap_or_default()
    }

    /// Shut down, and require the server to have ended cleanly.
    pub(crate) fn goodbye(mut self) {
        self.ask("shutdown", json!(null)).expect("shutdown must be answered");
        self.tell("exit", json!(null));
        // The writer at the far end only leaves its loop once every sender is gone, so
        // the door closes before anything waits on what is behind it.
        let Self { outbox, ending, .. } = self;
        drop(outbox);
        match ending {
            Ending::Thread(thread) => thread
                .join()
                .expect("the server thread must not panic")
                .expect("the server must end without an error"),
            Ending::Process(mut child) => {
                let status = child.wait().expect("the server process must be reapable");
                assert!(status.success(), "`zone lsp --stdio` ended with {status}");
            }
        }
    }

    fn post(&mut self, message: Message) {
        match &mut self.outbox {
            Outbox::Memory(connection) => {
                connection.sender.send(message).expect("the server must be listening");
            }
            Outbox::Pipe(stdin) => {
                message.write(stdin).expect("the server's input must be open");
                stdin.flush().expect("the server's input must accept a flush");
            }
        }
    }

    fn next_message(&self) -> Message {
        self.inbox.recv_timeout(PATIENCE).expect("the server must answer within the deadline")
    }
}

/// Drain a blocking source into a channel, so every read upstream can carry a deadline.
fn pump(mut next: impl FnMut() -> Option<Message> + Send + 'static) -> Receiver<Message> {
    let (sender, inbox) = mpsc::channel();
    thread::spawn(move || {
        while let Some(message) = next() {
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    inbox
}

/// A `file://` URI for a path that exists on disk, which is what the architecture
/// diagnostics need in order to find the contract that governs a buffer.
pub(crate) fn uri_of(path: &std::path::Path) -> String {
    let absolute = path.canonicalize().expect("the fixture must exist on disk");
    format!("file://{}", absolute.display())
}

/// Where the shared fixture packages live, from wherever the test binary runs.
pub(crate) fn fixture(kind: &str, name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(kind)
        .join(name)
}

/// An adapter file under `editors/`, read as text. A missing one is a failure and not a
/// skip: an adapter that stops shipping a file has stopped being the adapter under test.
pub(crate) fn adapter(path: &str) -> String {
    let file =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../editors").join(path);
    std::fs::read_to_string(&file)
        .unwrap_or_else(|why| panic!("editors/{path} must be readable: {why}"))
}
