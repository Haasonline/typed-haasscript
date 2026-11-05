//! `ths server`: a language server over standard input and output. It
//! reports the diagnostics `ths check` would, as the owner types: every
//! open document is checked again after each change, seeing the other open
//! documents' unsaved text. Changes that arrive together are checked once.

mod completion;
mod document_analyses;
mod document_check;
mod document_paths;
mod file_watching;
mod hover_text;
mod navigation;
mod open_documents;
mod positions;
mod session;

#[cfg(test)]
mod tests;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::process::ExitCode;

use lsp_server::{Connection, Message, Notification};
use lsp_types::notification::{Exit, LogMessage, Notification as _};
use lsp_types::{
    CompletionOptions, HoverProviderCapability, InitializeParams, InitializeResult, LogMessageParams, MessageType,
    OneOf, ServerCapabilities, ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions,
};

use self::positions::PositionEncoding;
use self::session::Session;

const SERVER_NAME: &str = "ths";

pub fn run() -> Result<ExitCode, String> {
    let (connection, io_threads) = Connection::stdio();
    serve(&connection)?;
    // The writer thread ends once every sender is gone.
    drop(connection);
    io_threads.join().map_err(|error| format!("ths server: {error}"))?;
    Ok(ExitCode::SUCCESS)
}

/// Answers the editor on `connection` until it shuts the server down.
pub fn serve(connection: &Connection) -> Result<(), String> {
    let agreement = initialize(connection)?;
    if agreement.editor_watches_files {
        send(connection, Message::Request(file_watching::registration_request()))?;
    }
    let mut session = Session::new(agreement.encoding);
    while let Ok(first) = connection.receiver.recv() {
        let mut check_again = false;
        let mut next = Some(first);
        while let Some(message) = next {
            match message {
                Message::Request(request) => {
                    if connection.handle_shutdown(&request).map_err(|error| error.to_string())? {
                        return Ok(());
                    }
                    // Answered from the text as it is now.
                    if check_again {
                        send_checked(connection, &mut session)?;
                        check_again = false;
                    }
                    send(connection, Message::Response(session.answer(request)))?;
                }
                Message::Notification(notification) if notification.method == Exit::METHOD => return Ok(()),
                Message::Notification(notification) => {
                    let reaction = session.handle(notification);
                    check_again |= reaction.check_again;
                    for message in reaction.messages {
                        send(connection, Message::Notification(message))?;
                    }
                }
                Message::Response(_) => {}
            }
            next = connection.receiver.try_recv().ok();
        }
        if check_again {
            send_checked(connection, &mut session)?;
        }
    }
    Ok(())
}

fn send_checked(connection: &Connection, session: &mut Session) -> Result<(), String> {
    for message in checked(session) {
        send(connection, Message::Notification(message))?;
    }
    Ok(())
}

/// What the editor and the server agreed on at start-up.
struct Agreement {
    encoding: PositionEncoding,
    editor_watches_files: bool,
}

/// Agrees on the position encoding and tells the editor what the server
/// does.
fn initialize(connection: &Connection) -> Result<Agreement, String> {
    let (id, parameters) = connection.initialize_start().map_err(|error| error.to_string())?;
    let parameters: InitializeParams = serde_json::from_value(parameters).map_err(|error| error.to_string())?;
    let editor_watches_files = file_watching::editor_watches_files(&parameters.capabilities);
    let offered = parameters.capabilities.general.and_then(|general| general.position_encodings);
    let encoding = PositionEncoding::chosen_from(offered.as_deref());
    let result = InitializeResult {
        capabilities: server_capabilities(encoding),
        server_info: Some(ServerInfo {
            name: SERVER_NAME.to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
        }),
    };
    let result = serde_json::to_value(result).map_err(|error| error.to_string())?;
    connection.initialize_finish(id, result).map_err(|error| error.to_string())?;
    Ok(Agreement { encoding, editor_watches_files })
}

/// What the server does, using `encoding` for positions.
fn server_capabilities(encoding: PositionEncoding) -> ServerCapabilities {
    let synchronization = TextDocumentSyncOptions {
        open_close: Some(true),
        change: Some(TextDocumentSyncKind::FULL),
        save: Some(TextDocumentSyncSaveOptions::Supported(true)),
        ..TextDocumentSyncOptions::default()
    };
    ServerCapabilities {
        position_encoding: Some(encoding.kind()),
        text_document_sync: Some(TextDocumentSyncCapability::Options(synchronization)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(completion::TRIGGER_CHARACTERS.map(String::from).to_vec()),
            ..CompletionOptions::default()
        }),
        ..ServerCapabilities::default()
    }
}

/// The messages of a check; a check that panics is logged, and the
/// server goes on with the next change.
fn checked(session: &mut Session) -> Vec<Notification> {
    catch_unwind(AssertUnwindSafe(|| session.check())).unwrap_or_else(|_| {
        let message = "ths server: checking failed; see the server's error output".to_string();
        let parameters = LogMessageParams { typ: MessageType::ERROR, message };
        vec![Notification::new(LogMessage::METHOD.to_string(), parameters)]
    })
}

fn send(connection: &Connection, message: Message) -> Result<(), String> {
    connection.sender.send(message).map_err(|error| format!("ths server: the editor went away: {error}"))
}
