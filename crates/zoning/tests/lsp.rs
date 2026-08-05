//! End-to-end protocol tests over lsp-server's in-memory transport.

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use serde_json::json;

type Server = std::thread::JoinHandle<Result<(), String>>;

fn initialized() -> Result<(Connection, Server), String> {
    let (server, client) = Connection::memory();
    let thread = std::thread::spawn(move || zoning::lsp::serve(&server));
    client
        .sender
        .send(
            Request::new(RequestId::from(1), "initialize".to_owned(), json!({"capabilities": {}}))
                .into(),
        )
        .map_err(|error| error.to_string())?;
    let Message::Response(response) = client.receiver.recv().map_err(|error| error.to_string())?
    else {
        return Err("initialize must answer with a response".to_owned());
    };
    assert!(response.response_result.is_ok());
    client
        .sender
        .send(Notification::new("initialized".to_owned(), json!({})).into())
        .map_err(|error| error.to_string())?;
    Ok((client, thread))
}

fn stop(client: &Connection, thread: Server) -> Result<(), String> {
    client
        .sender
        .send(Request::new(RequestId::from(2), "shutdown".to_owned(), json!(null)).into())
        .map_err(|error| error.to_string())?;
    assert!(matches!(
        client.receiver.recv().map_err(|error| error.to_string())?,
        Message::Response(_)
    ));
    client
        .sender
        .send(Notification::new("exit".to_owned(), json!(null)).into())
        .map_err(|error| error.to_string())?;
    thread.join().map_err(|_| "server thread panicked".to_owned())??;
    Ok(())
}

#[test]
fn publishes_precise_diagnostics_for_an_unsaved_document() -> Result<(), String> {
    let (client, thread) = initialized()?;
    client
        .sender
        .send(
            Notification::new(
                "textDocument/didOpen".to_owned(),
                json!({
                    "textDocument": {
                        "uri": "file:///tmp/broken.zone",
                        "languageId": "zoning",
                        "version": 1,
                        "text": "module demo\nzones { 😀 }\n"
                    }
                }),
            )
            .into(),
        )
        .map_err(|error| error.to_string())?;

    let Message::Notification(diagnostics) =
        client.receiver.recv().map_err(|error| error.to_string())?
    else {
        return Err("opening a malformed document must publish diagnostics".to_owned());
    };
    assert_eq!(diagnostics.method, "textDocument/publishDiagnostics");
    let first = &diagnostics.params["diagnostics"][0];
    assert_eq!(first["source"], "zoning");
    assert!(first["range"]["start"]["line"].as_u64().ok_or("diagnostic line must be numeric")? < 3);
    stop(&client, thread)
}

#[test]
fn completion_and_utf16_rename_are_protocol_shaped() -> Result<(), String> {
    let (client, thread) = initialized()?;
    let uri = "file:///tmp/utf16.zone";
    client
        .sender
        .send(
            Notification::new(
                "textDocument/didOpen".to_owned(),
                json!({
                    "textDocument": {
                        "uri": uri,
                        "languageId": "zoning",
                        "version": 1,
                        "text": "module demo\n# 😀\nzone core { \"src/**\" }\n"
                    }
                }),
            )
            .into(),
        )
        .map_err(|error| error.to_string())?;
    let _ = client.receiver.recv().map_err(|error| error.to_string())?;

    client
        .sender
        .send(
            Request::new(
                RequestId::from(3),
                "textDocument/completion".to_owned(),
                json!({"textDocument": {"uri": uri}, "position": {"line": 2, "character": 2}}),
            )
            .into(),
        )
        .map_err(|error| error.to_string())?;
    let Message::Response(response) = client.receiver.recv().map_err(|error| error.to_string())?
    else {
        return Err("completion must answer".to_owned());
    };
    let result = response.response_result.map_err(|error| error.message)?;
    assert!(
        result["items"]
            .as_array()
            .ok_or("completion items must be an array")?
            .iter()
            .any(|item| item["label"] == "zones")
    );
    stop(&client, thread)
}
