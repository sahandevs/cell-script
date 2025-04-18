use crate::parser::AST;
use dashmap::DashMap;
use log::debug;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

#[derive(Debug)]
struct Backend {
    client: Client,
    ast_map: DashMap<String, AST>,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            server_info: None,
            offset_encoding: None,
            capabilities: ServerCapabilities {
                inlay_hint_provider: Some(OneOf::Left(true)),
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::FULL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(true),
                        })),
                        ..Default::default()
                    },
                )),
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![".".to_string()]),
                    work_done_progress_options: Default::default(),
                    all_commit_characters: None,
                    completion_item: None,
                }),
                execute_command_provider: Some(ExecuteCommandOptions {
                    commands: vec!["dummy.do_something".to_string()],
                    work_done_progress_options: Default::default(),
                }),

                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(
                        SemanticTokensRegistrationOptions {
                            text_document_registration_options: {
                                TextDocumentRegistrationOptions {
                                    document_selector: Some(vec![DocumentFilter {
                                        language: Some("nrs".to_string()),
                                        scheme: Some("file".to_string()),
                                        pattern: None,
                                    }]),
                                }
                            },
                            semantic_tokens_options: SemanticTokensOptions {
                                work_done_progress_options: WorkDoneProgressOptions::default(),
                                legend: SemanticTokensLegend {
                                    token_types: [
                                        SemanticTokenType::PARAMETER,
                                        SemanticTokenType::NUMBER,
                                        SemanticTokenType::FUNCTION,
                                        SemanticTokenType::OPERATOR,
                                    ].into(),
                                    token_modifiers: vec![],
                                },
                                range: Some(true),
                                full: Some(SemanticTokensFullOptions::Bool(true)),
                            },
                            static_registration_options: StaticRegistrationOptions::default(),
                        },
                    ),
                ),
                // definition: Some(GotoCapability::default()),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
        })
    }
    async fn initialized(&self, _: InitializedParams) {
        debug!("initialized!");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        dbg!(&params.text);
        if let Some(text) = params.text {
            _ = self.client.semantic_tokens_refresh().await;
        }
        println!("file saved!");
    }
    async fn did_close(&self, _: DidCloseTextDocumentParams) {
        println!("file closed!");
    }

    // async fn goto_definition(
    //     &self,
    //     params: GotoDefinitionParams,
    // ) -> Result<Option<GotoDefinitionResponse>> {
    //     let definition = || -> Option<GotoDefinitionResponse> {
    //         let uri = params.text_document_position_params.text_document.uri;
    //         let semantic = self.semantic_map.get(uri.as_str())?;
    //         let rope = self.document_map.get(uri.as_str())?;
    //         let position = params.text_document_position_params.position;
    //         let offset = position_to_offset(position, &rope)?;

    //         let interval = semantic.ident_range.find(offset, offset + 1).next()?;
    //         let interval_val = interval.val;
    //         let range = match interval_val {
    //             IdentType::Binding(symbol_id) => {
    //                 let span = &semantic.table.symbol_id_to_span[symbol_id];
    //                 Some(span.clone())
    //             }
    //             IdentType::Reference(reference_id) => {
    //                 let reference = semantic.table.reference_id_to_reference.get(reference_id)?;
    //                 let symbol_id = reference.symbol_id?;
    //                 let symbol_range = semantic.table.symbol_id_to_span.get(symbol_id)?;
    //                 Some(symbol_range.clone())
    //             }
    //         };

    //         range.and_then(|range| {
    //             let start_position = offset_to_position(range.start, &rope)?;
    //             let end_position = offset_to_position(range.end, &rope)?;
    //             Some(GotoDefinitionResponse::Scalar(Location::new(
    //                 uri,
    //                 Range::new(start_position, end_position),
    //             )))
    //         })
    //     }();
    //     Ok(definition)
    // }

    // async fn inlay_hint(
    //     &self,
    //     params: tower_lsp::lsp_types::InlayHintParams,
    // ) -> Result<Option<Vec<InlayHint>>> {
    //     debug!("inlay hint");
    //     let uri = &params.text_document.uri;
    //     let mut hashmap = HashMap::new();
    //     if let Some(ast) = self.ast_map.get(uri.as_str()) {
    //         ast.iter().for_each(|(func, _)| {
    //             type_inference(&func.body, &mut hashmap);
    //         });
    //     }

    //     let document = match self.document_map.get(uri.as_str()) {
    //         Some(rope) => rope,
    //         None => return Ok(None),
    //     };
    //     let inlay_hint_list = hashmap
    //         .into_iter()
    //         .map(|(k, v)| {
    //             (
    //                 k.start,
    //                 k.end,
    //                 match v {
    //                     nrs_language_server::nrs_lang::Value::Null => "null".to_string(),
    //                     nrs_language_server::nrs_lang::Value::Bool(_) => "bool".to_string(),
    //                     nrs_language_server::nrs_lang::Value::Num(_) => "number".to_string(),
    //                     nrs_language_server::nrs_lang::Value::Str(_) => "string".to_string(),
    //                 },
    //             )
    //         })
    //         .filter_map(|item| {
    //             // let start_position = offset_to_position(item.0, document)?;
    //             let end_position = offset_to_position(item.1, &document)?;
    //             let inlay_hint = InlayHint {
    //                 text_edits: None,
    //                 tooltip: None,
    //                 kind: Some(InlayHintKind::TYPE),
    //                 padding_left: None,
    //                 padding_right: None,
    //                 data: None,
    //                 position: end_position,
    //                 label: InlayHintLabel::LabelParts(vec![InlayHintLabelPart {
    //                     value: item.2,
    //                     tooltip: None,
    //                     location: Some(Location {
    //                         uri: params.text_document.uri.clone(),
    //                         range: Range {
    //                             start: Position::new(0, 4),
    //                             end: Position::new(0, 10),
    //                         },
    //                     }),
    //                     command: None,
    //                 }]),
    //             };
    //             Some(inlay_hint)
    //         })
    //         .collect::<Vec<_>>();

    //     Ok(Some(inlay_hint_list))
    // }

    // async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
    //     let uri = params.text_document_position.text_document.uri;
    //     let position = params.text_document_position.position;
    //     let completions = || -> Option<Vec<CompletionItem>> {
    //         let rope = self.document_map.get(&uri.to_string())?;
    //         let ast = self.ast_map.get(&uri.to_string())?;
    //         let char = rope.try_line_to_char(position.line as usize).ok()?;
    //         let offset = char + position.character as usize;
    //         let completions = completion(&ast, offset);
    //         let mut ret = Vec::with_capacity(completions.len());
    //         for (_, item) in completions {
    //             match item {
    //                 nrs_language_server::completion::ImCompleteCompletionItem::Variable(var) => {
    //                     ret.push(CompletionItem {
    //                         label: var.clone(),
    //                         insert_text: Some(var.clone()),
    //                         kind: Some(CompletionItemKind::VARIABLE),
    //                         detail: Some(var),
    //                         ..Default::default()
    //                     });
    //                 }
    //                 nrs_language_server::completion::ImCompleteCompletionItem::Function(
    //                     name,
    //                     args,
    //                 ) => {
    //                     ret.push(CompletionItem {
    //                         label: name.clone(),
    //                         kind: Some(CompletionItemKind::FUNCTION),
    //                         detail: Some(name.clone()),
    //                         insert_text: Some(format!(
    //                             "{}({})",
    //                             name,
    //                             args.iter()
    //                                 .enumerate()
    //                                 .map(|(index, item)| { format!("${{{}:{}}}", index + 1, item) })
    //                                 .collect::<Vec<_>>()
    //                                 .join(",")
    //                         )),
    //                         insert_text_format: Some(InsertTextFormat::SNIPPET),
    //                         ..Default::default()
    //                     });
    //                 }
    //             }
    //         }
    //         Some(ret)
    //     }();
    //     Ok(completions.map(CompletionResponse::Array))
    // }
}

#[tokio::main]
pub async fn start_lsp() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::build(|client| Backend {
        client,
        ast_map: DashMap::new(),
    })
    .finish();

    Server::new(stdin, stdout, socket).serve(service).await;
}
