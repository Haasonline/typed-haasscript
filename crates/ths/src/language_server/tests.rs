//! The server driven as an editor drives it, over an in-memory connection.

use std::path::Path;
use std::str::FromStr;
use std::thread::JoinHandle;
use std::time::Duration;

use lsp_server::{Connection, Message, Notification, Request, RequestId};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument, Exit, Initialized,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    Completion, GotoDefinition, HoverRequest, Initialize, RegisterCapability, Request as _, Shutdown,
};
use lsp_types::{
    ClientCapabilities, CompletionItem, CompletionItemKind, CompletionResponse, DidChangeTextDocumentParams,
    DidChangeWatchedFilesClientCapabilities, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    GotoDefinitionResponse, Hover, HoverContents, InitializeParams, InitializeResult, InitializedParams, Location,
    PositionEncodingKind, PublishDiagnosticsParams, RegistrationParams, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier, WorkspaceClientCapabilities,
};
use url::Url;

use super::serve;

const LANGUAGE: &str = "typed-haasscript";
const WAIT: Duration = Duration::from_secs(20);

struct Editor {
    connection: Connection,
    server: Option<JoinHandle<Result<(), String>>>,
    next_id: i32,
}

impl Editor {
    fn start() -> (Editor, InitializeResult) {
        Editor::start_with(ClientCapabilities::default())
    }

    fn start_with(capabilities: ClientCapabilities) -> (Editor, InitializeResult) {
        let (server_side, connection) = Connection::memory();
        let server = std::thread::spawn(move || serve(&server_side));
        let mut editor = Editor { connection, server: Some(server), next_id: 0 };
        let parameters = InitializeParams { capabilities, ..InitializeParams::default() };
        let result = editor.request(Initialize::METHOD, parameters);
        editor.notify(Initialized::METHOD, InitializedParams {});
        (editor, serde_json::from_value(result).unwrap())
    }

    fn request(&mut self, method: &str, parameters: impl serde::Serialize) -> serde_json::Value {
        self.next_id += 1;
        let id = RequestId::from(self.next_id);
        let request = Request::new(id.clone(), method.to_string(), parameters);
        self.connection.sender.send(Message::Request(request)).unwrap();
        loop {
            match self.connection.receiver.recv_timeout(WAIT).unwrap() {
                Message::Response(response) if response.id == id => return response.response_result.unwrap(),
                _ => {}
            }
        }
    }

    fn notify(&self, method: &str, parameters: impl serde::Serialize) {
        let notification = Notification::new(method.to_string(), parameters);
        self.connection.sender.send(Message::Notification(notification)).unwrap();
    }

    fn open(&self, uri: &Uri, text: &str) {
        let text_document = TextDocumentItem::new(uri.clone(), LANGUAGE.to_string(), 1, text.to_string());
        self.notify(DidOpenTextDocument::METHOD, DidOpenTextDocumentParams { text_document });
    }

    fn change(&self, uri: &Uri, version: i32, text: &str) {
        let parameters = DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier::new(uri.clone(), version),
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        };
        self.notify(DidChangeTextDocument::METHOD, parameters);
    }

    fn close(&self, uri: &Uri) {
        let text_document = TextDocumentIdentifier::new(uri.clone());
        self.notify(DidCloseTextDocument::METHOD, DidCloseTextDocumentParams { text_document });
    }

    /// The next diagnostics the server sends for `uri`.
    fn diagnostics_of(&self, uri: &Uri) -> PublishDiagnosticsParams {
        loop {
            if let Message::Notification(notification) = self.connection.receiver.recv_timeout(WAIT).unwrap()
                && notification.method == PublishDiagnostics::METHOD
            {
                let published: PublishDiagnosticsParams = serde_json::from_value(notification.params).unwrap();
                if &published.uri == uri {
                    return published;
                }
            }
        }
    }

    fn messages_of(&self, uri: &Uri) -> Vec<String> {
        self.diagnostics_of(uri).diagnostics.into_iter().map(|diagnostic| diagnostic.message).collect()
    }

    fn stop(mut self) {
        self.request(Shutdown::METHOD, ());
        self.notify(Exit::METHOD, ());
        self.server.take().unwrap().join().unwrap().unwrap();
    }
}

fn uri_of(path: &Path) -> Uri {
    Uri::from_str(Url::from_file_path(path).unwrap().as_str()).unwrap()
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for (path, source) in files {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    directory
}

#[test]
fn positions_are_utf16_unless_the_editor_offers_utf8() {
    let (editor, result) = Editor::start();
    assert_eq!(result.capabilities.position_encoding, Some(PositionEncodingKind::UTF16));
    editor.stop();
}

#[test]
fn a_type_error_is_reported_where_it_is_and_goes_once_fixed() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (editor, _) = Editor::start();
    editor.open(&bot, "local a = 1\nlocal b: string = a");
    let published = editor.diagnostics_of(&bot);
    assert_eq!(published.version, Some(1));
    let error = &published.diagnostics[0];
    assert_eq!(error.message, "Type 'number' is not assignable to type 'string'.");
    assert_eq!((error.range.start.line, error.range.start.character), (1, 18));
    editor.change(&bot, 2, "local a = \"text\"\nlocal b: string = a");
    assert_eq!(editor.messages_of(&bot), Vec::<String>::new());
    editor.stop();
}

#[test]
fn a_script_sees_the_unsaved_exports_of_an_open_module() {
    let project = project(&[("ths.toml", ""), ("bot.ths", ""), ("quotes.ths", "export local bid = 1")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let quotes = uri_of(&project.path().join("quotes.ths"));
    let (editor, _) = Editor::start();
    editor.open(&bot, "import { bid } from \"./quotes\"\nlocal text: string = bid");
    assert_eq!(editor.messages_of(&bot), ["Type 'number' is not assignable to type 'string'."]);
    editor.open(&quotes, "export local bid = \"unsaved\"");
    assert_eq!(editor.messages_of(&bot), Vec::<String>::new());
    editor.close(&quotes);
    assert_eq!(editor.messages_of(&quotes), Vec::<String>::new());
    assert_eq!(editor.messages_of(&bot), ["Type 'number' is not assignable to type 'string'."]);
    editor.stop();
}

#[test]
fn the_stripped_output_beside_a_typed_file_is_not_checked() {
    let project = project(&[("bot.ths", "Log(1)"), ("bot.hs", "")]);
    let stripped = uri_of(&project.path().join("bot.hs"));
    let (editor, _) = Editor::start();
    editor.open(&stripped, "Log(undefinedName)");
    assert_eq!(editor.messages_of(&stripped), Vec::<String>::new());
    editor.stop();
}

#[test]
fn an_editor_that_takes_it_is_asked_to_watch_the_files_checking_reads() {
    let watching = DidChangeWatchedFilesClientCapabilities { dynamic_registration: Some(true), ..Default::default() };
    let workspace = WorkspaceClientCapabilities { did_change_watched_files: Some(watching), ..Default::default() };
    let (editor, _) = Editor::start_with(ClientCapabilities { workspace: Some(workspace), ..Default::default() });
    let registration = loop {
        if let Message::Request(request) = editor.connection.receiver.recv_timeout(WAIT).unwrap()
            && request.method == RegisterCapability::METHOD
        {
            break serde_json::from_value::<RegistrationParams>(request.params).unwrap();
        }
    };
    let options = registration.registrations[0].register_options.as_ref().unwrap().to_string();
    assert!(options.contains("**/*.{ths,hs,lua}") && options.contains("**/ths.toml"), "{options}");
    editor.stop();
}

fn place(uri: &Uri, line: u32, character: u32) -> serde_json::Value {
    serde_json::json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": character } })
}

impl Editor {
    fn hover(&mut self, uri: &Uri, line: u32, character: u32) -> Option<Hover> {
        serde_json::from_value(self.request(HoverRequest::METHOD, place(uri, line, character))).unwrap()
    }

    fn definition(&mut self, uri: &Uri, line: u32, character: u32) -> Option<Location> {
        let response: Option<GotoDefinitionResponse> =
            serde_json::from_value(self.request(GotoDefinition::METHOD, place(uri, line, character))).unwrap();
        match response? {
            GotoDefinitionResponse::Scalar(location) => Some(location),
            other => panic!("one location expected, not {other:?}"),
        }
    }
}

fn markdown(hover: Hover) -> String {
    match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        other => panic!("markdown expected, not {other:?}"),
    }
}

fn start_of(location: &Location) -> (u32, u32) {
    (location.range.start.line, location.range.start.character)
}

#[test]
fn hover_and_definition_are_advertised() {
    let (editor, result) = Editor::start();
    assert!(result.capabilities.hover_provider.is_some());
    assert!(result.capabilities.definition_provider.is_some());
    editor.stop();
}

#[test]
fn hovering_a_name_shows_its_declaration_type_and_documentation() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "-- How far back to look.\nlocal period = 14\nLog(period)");
    let hover = editor.hover(&bot, 2, 5).unwrap();
    assert_eq!(markdown(hover), "```typed-haasscript\nlocal period: number\n```\n\n---\n\nHow far back to look.");
    assert_eq!(editor.hover(&bot, 2, 0).map(markdown).map(|text| text.contains("Log")), Some(true));
    editor.stop();
}

#[test]
fn a_request_after_a_change_is_answered_from_the_changed_text() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local period = 14\nLog(period)");
    editor.change(&bot, 2, "local period = \"long\"\nLog(period)");
    let hover = markdown(editor.hover(&bot, 1, 5).unwrap());
    assert!(hover.contains("local period: string"), "{hover}");
    editor.stop();
}

#[test]
fn a_local_leads_to_its_declaration() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "\nlocal period = 14\nLog(period)");
    let location = editor.definition(&bot, 2, 6).unwrap();
    assert_eq!(location.uri, bot);
    assert_eq!(start_of(&location), (1, 6));
    assert_eq!(editor.definition(&bot, 0, 0), None);
    editor.stop();
}

#[test]
fn an_imported_name_leads_to_the_unsaved_export_in_its_module() {
    let project = project(&[("ths.toml", ""), ("bot.ths", ""), ("quotes.ths", "export local bid = 1")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let quotes = uri_of(&project.path().join("quotes.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&quotes, "\n\nexport local bid = 1");
    editor.open(&bot, "import { bid } from \"./quotes\"\nLog(bid)");
    let location = editor.definition(&bot, 1, 5).unwrap();
    assert_eq!(location.uri, quotes);
    assert_eq!(start_of(&location), (2, 13));
    editor.stop();
}

impl Editor {
    fn completions(&mut self, uri: &Uri, line: u32, character: u32) -> Vec<CompletionItem> {
        let response: Option<CompletionResponse> =
            serde_json::from_value(self.request(Completion::METHOD, place(uri, line, character))).unwrap();
        match response.unwrap() {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        }
    }
}

fn labelled<'a>(items: &'a [CompletionItem], label: &str) -> Option<&'a CompletionItem> {
    items.iter().find(|item| item.label == label)
}

#[test]
fn completion_is_advertised_for_members() {
    let (editor, result) = Editor::start();
    let triggers = result.capabilities.completion_provider.unwrap().trigger_characters.unwrap();
    assert_eq!(triggers, [".", ":"]);
    editor.stop();
}

#[test]
fn the_names_in_scope_are_offered_where_a_name_can_go() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local period = 14\nif period > 1 then\n  local inner = 2\nend\n\nlocal later = 3");
    let items = editor.completions(&bot, 4, 0);
    let period = labelled(&items, "period").unwrap();
    assert_eq!((period.kind, period.detail.as_deref()), (Some(CompletionItemKind::VARIABLE), Some("number")));
    assert!(labelled(&items, "inner").is_none());
    assert!(labelled(&items, "later").is_none());
    assert_eq!(labelled(&items, "Log").and_then(|item| item.kind), Some(CompletionItemKind::FUNCTION));
    editor.stop();
}

#[test]
fn the_fields_and_methods_of_a_value_are_offered_after_its_dot() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local quote = { bid = 1, Spread = function() return 0 end }\nLog(quote.)");
    let items = editor.completions(&bot, 1, 10);
    let mut labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    labels.sort();
    assert_eq!(labels, ["Spread", "bid"]);
    assert_eq!(labelled(&items, "bid").unwrap().kind, Some(CompletionItemKind::FIELD));
    assert_eq!(labelled(&items, "Spread").unwrap().kind, Some(CompletionItemKind::METHOD));
    editor.stop();
}

#[test]
fn nothing_is_offered_inside_a_comment_or_string() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local period = 14 -- per\nLog(\"per\")");
    assert_eq!(editor.completions(&bot, 0, 24), Vec::new());
    assert_eq!(editor.completions(&bot, 1, 8), Vec::new());
    assert!(labelled(&editor.completions(&bot, 1, 4), "period").is_some());
    editor.stop();
}

#[test]
fn only_methods_are_offered_after_a_colon() {
    let project = project(&[("ths.toml", ""), ("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local quote = { bid = 1, Spread = function(self) return 0 end }\nLog(quote:)");
    let labels: Vec<String> = editor.completions(&bot, 1, 10).into_iter().map(|item| item.label).collect();
    assert_eq!(labels, ["Spread"]);
    editor.stop();
}

#[test]
fn a_file_outside_any_project_is_not_checked() {
    let project = project(&[("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (mut editor, _) = Editor::start();
    editor.open(&bot, "local text: string = 1");
    assert_eq!(editor.messages_of(&bot), Vec::<String>::new());
    assert_eq!(editor.hover(&bot, 0, 7), None);
    editor.stop();
}

#[test]
fn creating_a_settings_file_starts_checking() {
    let project = project(&[("bot.ths", "")]);
    let bot = uri_of(&project.path().join("bot.ths"));
    let (editor, _) = Editor::start();
    editor.open(&bot, "local text: string = 1");
    assert_eq!(editor.messages_of(&bot), Vec::<String>::new());
    std::fs::write(project.path().join("ths.toml"), "").unwrap();
    editor.notify(DidChangeWatchedFiles::METHOD, serde_json::json!({ "changes": [] }));
    assert_eq!(editor.messages_of(&bot), ["Type '1' is not assignable to type 'string'."]);
    editor.stop();
}
