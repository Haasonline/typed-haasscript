//! What the server knows between messages, how each notification from the
//! editor changes it, and how each request is answered. Sending is left to
//! the caller.

use lsp_server::{ErrorCode, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    LogMessage, Notification as _, PublishDiagnostics,
};
use lsp_types::request::{Completion, GotoDefinition, HoverRequest, Request as _};
use lsp_types::{
    CompletionParams, CompletionResponse, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, GotoDefinitionParams, GotoDefinitionResponse, HoverParams, LogMessageParams,
    MessageType, PublishDiagnosticsParams,
};
use serde::de::DeserializeOwned;

use super::completion;
use super::document_analyses::DocumentAnalyses;
use super::document_check;
use super::document_paths::file_path;
use super::navigation;
use super::open_documents::{OpenDocument, OpenDocuments};
use super::positions::PositionEncoding;

pub struct Session {
    documents: OpenDocuments,
    /// From the latest check, for answering requests.
    analyses: DocumentAnalyses,
    encoding: PositionEncoding,
    /// The problems last logged, so the same ones are not logged on every
    /// change.
    logged_problems: Vec<String>,
}

/// What a notification calls for.
#[derive(Debug, Default)]
pub struct Reaction {
    /// Every open document must be checked again.
    pub check_again: bool,
    /// Messages to send now, before any check.
    pub messages: Vec<Notification>,
}

impl Session {
    pub fn new(encoding: PositionEncoding) -> Session {
        Session {
            documents: OpenDocuments::default(),
            analyses: DocumentAnalyses::default(),
            encoding,
            logged_problems: Vec::new(),
        }
    }

    pub fn handle(&mut self, notification: Notification) -> Reaction {
        let method = notification.method.as_str();
        if method == DidOpenTextDocument::METHOD {
            self.opened(parameters(notification.params))
        } else if method == DidChangeTextDocument::METHOD {
            self.changed(parameters(notification.params))
        } else if method == DidCloseTextDocument::METHOD {
            self.closed(parameters(notification.params))
        } else if method == DidSaveTextDocument::METHOD || method == DidChangeWatchedFiles::METHOD {
            // A saved or changed file can be a module, a command or a
            // declaration file other documents see.
            Reaction { check_again: true, messages: Vec::new() }
        } else {
            Reaction::default()
        }
    }

    /// Checks every open document: a diagnostics message for each, and a
    /// log message when the problems differ from the last ones logged.
    pub fn check(&mut self) -> Vec<Notification> {
        let checked = document_check::check(&self.documents, self.encoding);
        self.analyses.replace_all(checked.analyses);
        let mut messages: Vec<Notification> = checked.published.into_iter().map(published).collect();
        if checked.problems != self.logged_problems {
            if !checked.problems.is_empty() {
                let text = checked.problems.join("\n");
                let parameters = LogMessageParams { typ: MessageType::WARNING, message: text };
                messages.push(Notification::new(LogMessage::METHOD.to_string(), parameters));
            }
            self.logged_problems = checked.problems;
        }
        messages
    }

    /// Answers a request from the latest check; the caller checks first
    /// when changes are waiting.
    pub fn answer(&self, request: Request) -> Response {
        let method = request.method.as_str();
        if method == HoverRequest::METHOD {
            self.hover(request)
        } else if method == GotoDefinition::METHOD {
            self.definition(request)
        } else if method == Completion::METHOD {
            self.completion(request)
        } else {
            Response::new_err(request.id, ErrorCode::MethodNotFound as i32, request.method)
        }
    }

    fn hover(&self, request: Request) -> Response {
        let hover = parameters::<HoverParams>(request.params).and_then(|parameters| {
            let place = parameters.text_document_position_params;
            let document = self.analyses.get(&place.text_document.uri)?;
            navigation::hover(document, place.position, self.encoding)
        });
        Response::new_ok(request.id, hover)
    }

    fn definition(&self, request: Request) -> Response {
        let location = parameters::<GotoDefinitionParams>(request.params).and_then(|parameters| {
            let place = parameters.text_document_position_params;
            let document = self.analyses.get(&place.text_document.uri)?;
            navigation::definition(document, place.position, self.encoding, &self.documents)
        });
        Response::new_ok(request.id, location.map(GotoDefinitionResponse::Scalar))
    }

    fn completion(&self, request: Request) -> Response {
        let items = parameters::<CompletionParams>(request.params).and_then(|parameters| {
            let place = parameters.text_document_position;
            let document = self.analyses.get(&place.text_document.uri)?;
            Some(completion::completion(document, place.position, self.encoding))
        });
        Response::new_ok(request.id, items.map(CompletionResponse::Array))
    }

    fn opened(&mut self, parameters: Option<DidOpenTextDocumentParams>) -> Reaction {
        let Some(parameters) = parameters else { return Reaction::default() };
        let document = parameters.text_document;
        let Some(path) = file_path(&document.uri) else { return Reaction::default() };
        let opened = OpenDocument { uri: document.uri, version: document.version, text: document.text };
        self.documents.open(&path, opened);
        Reaction { check_again: true, messages: Vec::new() }
    }

    /// The server asks for whole documents, so the last change holds the
    /// full text.
    fn changed(&mut self, parameters: Option<DidChangeTextDocumentParams>) -> Reaction {
        let Some(parameters) = parameters else { return Reaction::default() };
        let Some(path) = file_path(&parameters.text_document.uri) else { return Reaction::default() };
        let Some(change) = parameters.content_changes.into_iter().last() else { return Reaction::default() };
        self.documents.change(&path, parameters.text_document.version, change.text);
        Reaction { check_again: true, messages: Vec::new() }
    }

    /// Clears the closed document's diagnostics; the others are checked
    /// again, since they now see the file as saved.
    fn closed(&mut self, parameters: Option<DidCloseTextDocumentParams>) -> Reaction {
        let Some(parameters) = parameters else { return Reaction::default() };
        let uri = parameters.text_document.uri;
        if let Some(path) = file_path(&uri) {
            self.documents.close(&path);
        }
        let cleared = PublishDiagnosticsParams { uri, diagnostics: Vec::new(), version: None };
        Reaction { check_again: true, messages: vec![published(cleared)] }
    }
}

fn published(parameters: PublishDiagnosticsParams) -> Notification {
    Notification::new(PublishDiagnostics::METHOD.to_string(), parameters)
}

/// `None` when the editor sent parameters that do not fit, which is
/// ignored rather than ending the session.
fn parameters<P: DeserializeOwned>(params: serde_json::Value) -> Option<P> {
    serde_json::from_value(params).ok()
}
