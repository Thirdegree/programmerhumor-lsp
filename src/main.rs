use lazy_static::lazy_static;
use regex::Regex;
use std::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::{lsp_types::*, Server};
use tower_lsp::{LanguageServer, LspService};

fn make_return_diagnostic(line_no: u32) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: line_no,
                character: 0,
            },
            end: Position {
                line: line_no,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::Number(3)),
        message: "All comments must return a value".to_string(),
        ..Default::default()
    }
}

fn make_semicolon_diagnostic(line_no: u32) -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: line_no,
                character: 0,
            },
            end: Position {
                line: line_no,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::Number(3)),
        message: "For comments, every sentence must end with a semicolon".to_string(),
        ..Default::default()
    }
}

fn make_import_diagnostic() -> Diagnostic {
    Diagnostic {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::Number(2)),
        message: "All posts and comments should start with an \"import\" declaration.".to_string(),
        ..Default::default()
    }
}

fn make_link_rick_roll_diagnostic() -> Diagnostic {
    Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::Number(4)),
                message: "Every post linking to something must contain a second, identical-looking link to a rick-roll".to_string(),
                ..Default::default()
            }
}

/// Implement the current rules for styling an r/programmerhumor comment
///     1. All post titles must be in camelCase
///         Ignored for now, this lsp looks at comment bodies
///     2. All posts and comments should start with an "import" declaration.
///     3. All comments must return a value
///     4. For comments, every sentence must end with a semicolon
///     5. Every post linking to something must contain a second, identical-looking link to a rick-roll
struct Backend {
    client: tower_lsp::Client,
    content: RwLock<String>,
}

impl Backend {
    fn compute_diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];
        let content = self.content.read().unwrap();
        let mut content_lines = content.lines().peekable();
        // rule 2
        if let Some(first_line) = content_lines.next() {
            lazy_static! {
                static ref IMPORT_MATCH: Regex = Regex::new(r"(?i)\bimport\b").unwrap(); // case-insensitive
            }
            if !IMPORT_MATCH.is_match(first_line) {
                diagnostics.push(make_import_diagnostic());
            }
            if content_lines.peek().is_none() {
                // import line is last line, MUST be missing return
                diagnostics.push(make_return_diagnostic(0))
            }
        }
        let mut line_no = 1;
        while let Some(line) = content_lines.next() {
            if content_lines.peek().is_some() {
                lazy_static! {
                    // either a non-space then a period then a space, OR anything then anything
                    // other than a semicolon then end of line
                    // this is a bit iffy, because it also flags e.g. numbered lists. but idk how
                    // the automod config looks, so we'll go with it.
                    static ref SENTENCE_END_MATCH: Regex = Regex::new(r"\w\.\s|.+[^;]$").unwrap();
                }
                if SENTENCE_END_MATCH.is_match(line) {
                    diagnostics.push(make_semicolon_diagnostic(line_no))
                }
            } else {
                lazy_static! {
                    static ref RETURN_MATCH: Regex = Regex::new(r"(?i)\breturn\b").unwrap(); // case-insensitive
                }
                if !RETURN_MATCH.is_match(line) {
                    diagnostics.push(make_return_diagnostic(line_no))
                }
            }
            line_no += 1;
        }
        lazy_static! {
            // SHOULD match anything like [link text](https://url.com)
            // Technically we should also be checking the link text is the same, but lazy atm.
            // Maybe later
            static ref MARKDOWN_LINK_MATCH: Regex = Regex::new(r"\[[^]]+\]\(([^)]+)\)").unwrap();
        }
        if MARKDOWN_LINK_MATCH.is_match(&content)
            && !MARKDOWN_LINK_MATCH
                .captures_iter(&content)
                .any(|captures| &captures[1] == r"https://www.youtube.com/watch?v=dQw4w9WgXcQ")
        {
            diagnostics.push(make_link_rick_roll_diagnostic())
        }

        diagnostics
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),

                ..Default::default()
            },
            ..Default::default()
        })
    }
    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        *self.content.write().unwrap() = params.text_document.text;
        self.client
            .publish_diagnostics(params.text_document.uri, self.compute_diagnostics(), None)
            .await;
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        *self.content.write().unwrap() = params.content_changes.first().unwrap().text.clone();
        self.client
            .publish_diagnostics(params.text_document.uri, self.compute_diagnostics(), None)
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        content: String::new().into(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
