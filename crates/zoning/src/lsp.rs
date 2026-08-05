//! Language Server Protocol support for `.zone` authoring.

mod diagnostics;
mod language;

use std::collections::HashMap;

use lsp_server::{Connection, Message, Notification};
use serde_json::{Value, json};

#[derive(Default)]
struct State {
    documents: HashMap<String, String>,
}

/// Serve LSP over standard input and output.
///
/// # Errors
/// Returns a transport or protocol error when the editor disconnects incorrectly.
pub fn serve_stdio() -> Result<(), String> {
    let (connection, threads) = Connection::stdio();
    let outcome = serve(&connection);
    // The writer thread exits its receive loop only once every `Sender` clone
    // is dropped. `connection` owns one, so it must go before `join` — held
    // open across the join, it deadlocks the process on a clean `exit`.
    drop(connection);
    outcome?;
    threads.join().map_err(|error| error.to_string())
}

/// Serve one connection. Public so protocol tests can use an in-memory transport.
///
/// # Errors
/// Returns a transport or protocol error when the peer violates the LSP handshake.
pub fn serve(connection: &Connection) -> Result<(), String> {
    let (id, _) = connection.initialize_start().map_err(|error| error.to_string())?;
    connection
        .initialize_finish(
            id,
            json!({
                "capabilities": language::capabilities(),
                "serverInfo": {"name": "zoning", "version": env!("CARGO_PKG_VERSION")}
            }),
        )
        .map_err(|error| error.to_string())?;

    let mut state = State::default();
    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request).map_err(|error| error.to_string())? {
                    break;
                }
                let response = language::respond(&state.documents, request);
                connection.sender.send(response.into()).map_err(|error| error.to_string())?;
            }
            Message::Notification(notification) => {
                if notification.method == "exit" {
                    break;
                }
                notify(connection, &mut state, &notification)?;
            }
            Message::Response(_) => {}
        }
    }
    Ok(())
}

fn notify(
    connection: &Connection,
    state: &mut State,
    notification: &Notification,
) -> Result<(), String> {
    match notification.method.as_str() {
        "textDocument/didOpen" => {
            let uri = pointer_str(&notification.params, "/textDocument/uri")?;
            let text = pointer_str(&notification.params, "/textDocument/text")?;
            state.documents.insert(uri.to_owned(), text.to_owned());
            diagnostics::publish(connection, uri, text)?;
        }
        "textDocument/didChange" => {
            let uri = pointer_str(&notification.params, "/textDocument/uri")?.to_owned();
            if let Some(text) =
                notification.params.pointer("/contentChanges/0/text").and_then(Value::as_str)
            {
                state.documents.insert(uri.clone(), text.to_owned());
                diagnostics::publish(connection, &uri, text)?;
            }
        }
        "textDocument/didSave" => {
            let uri = pointer_str(&notification.params, "/textDocument/uri")?;
            if let Some(text) = state.documents.get(uri) {
                diagnostics::publish(connection, uri, text)?;
            }
        }
        "textDocument/didClose" => {
            let uri = pointer_str(&notification.params, "/textDocument/uri")?;
            state.documents.remove(uri);
            diagnostics::clear(connection, uri)?;
        }
        "workspace/didChangeWatchedFiles" => {
            for (uri, text) in state.documents.clone() {
                diagnostics::publish(connection, &uri, &text)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn pointer_str<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, String> {
    value.pointer(pointer).and_then(Value::as_str).ok_or_else(|| format!("missing {pointer}"))
}
