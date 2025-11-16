use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde_json::{json, Value};

pub fn markdown_to_notion_blocks(markdown: &str) -> Vec<Value> {
    let parser = Parser::new(markdown);
    let mut blocks = Vec::new();
    let mut current_paragraph: Vec<Value> = Vec::new();
    let mut current_list_items: Vec<Value> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    let mut list_depth: usize = 0;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                let heading_level = match level {
                    pulldown_cmark::HeadingLevel::H1 => "heading_1",
                    pulldown_cmark::HeadingLevel::H2 => "heading_2",
                    pulldown_cmark::HeadingLevel::H3 => "heading_3",
                    _ => "heading_3",
                };
                blocks.push(json!({
                    "object": "block",
                    "type": heading_level,
                    heading_level: {
                        "rich_text": []
                    }
                }));
            }
            Event::End(TagEnd::Heading(_)) => {
                // Heading text is added via Text events
            }
            Event::Start(Tag::Paragraph) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
            }
            Event::End(TagEnd::Paragraph) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
            }
            Event::Start(Tag::List(_)) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                list_depth += 1;
            }
            Event::End(TagEnd::List(_)) => {
                flush_list(&mut current_list_items, &mut blocks);
                list_depth = list_depth.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                current_list_items.push(json!({
                    "object": "block",
                    "type": "bulleted_list_item",
                    "bulleted_list_item": {
                        "rich_text": []
                    }
                }));
            }
            Event::End(TagEnd::Item) => {
                // List item text is added via Text events
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                in_code_block = true;
                code_block_content.clear();
                code_block_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    pulldown_cmark::CodeBlockKind::Indented => "".to_string(),
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                blocks.push(json!({
                    "object": "block",
                    "type": "code",
                    "code": {
                        "rich_text": [{
                            "type": "text",
                            "text": {
                                "content": code_block_content.trim_end()
                            }
                        }],
                        "language": if code_block_lang.is_empty() { "plain text" } else { &code_block_lang }
                    }
                }));
                code_block_content.clear();
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                blocks.push(json!({
                    "object": "block",
                    "type": "quote",
                    "quote": {
                        "rich_text": []
                    }
                }));
            }
            Event::End(TagEnd::BlockQuote) => {}
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else if !current_list_items.is_empty() {
                    // Add to last list item
                    if let Some(item) = current_list_items.last_mut() {
                        if let Some(rich_text) = item["bulleted_list_item"]["rich_text"].as_array_mut() {
                            rich_text.push(json!({
                                "type": "text",
                                "text": {
                                    "content": text.to_string()
                                }
                            }));
                        }
                    }
                } else if !blocks.is_empty() {
                    // Check if we're adding to a heading or quote
                    if let Some(last_block) = blocks.last_mut() {
                        let block_type = last_block["type"].as_str().unwrap_or("").to_string();
                        if block_type.starts_with("heading_") {
                            if let Some(rich_text) = last_block[&block_type]["rich_text"].as_array_mut() {
                                rich_text.push(json!({
                                    "type": "text",
                                    "text": {
                                        "content": text.to_string()
                                    }
                                }));
                                continue;
                            }
                        } else if block_type == "quote" {
                            if let Some(rich_text) = last_block["quote"]["rich_text"].as_array_mut() {
                                rich_text.push(json!({
                                    "type": "text",
                                    "text": {
                                        "content": text.to_string()
                                    }
                                }));
                                continue;
                            }
                        }
                    }
                    // Otherwise add to current paragraph
                    current_paragraph.push(json!({
                        "type": "text",
                        "text": {
                            "content": text.to_string()
                        }
                    }));
                } else {
                    current_paragraph.push(json!({
                        "type": "text",
                        "text": {
                            "content": text.to_string()
                        }
                    }));
                }
            }
            Event::Code(code) => {
                let rich_text_item = json!({
                    "type": "text",
                    "text": {
                        "content": code.to_string()
                    },
                    "annotations": {
                        "code": true
                    }
                });

                if !current_list_items.is_empty() {
                    if let Some(item) = current_list_items.last_mut() {
                        if let Some(rich_text) = item["bulleted_list_item"]["rich_text"].as_array_mut() {
                            rich_text.push(rich_text_item);
                        }
                    }
                } else {
                    current_paragraph.push(rich_text_item);
                }
            }
            Event::Start(Tag::Strong) => {
                // Bold text - we'll handle this with annotations in a future iteration
            }
            Event::End(TagEnd::Strong) => {}
            Event::Start(Tag::Emphasis) => {
                // Italic text - we'll handle this with annotations in a future iteration
            }
            Event::End(TagEnd::Emphasis) => {}
            Event::Rule => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                blocks.push(json!({
                    "object": "block",
                    "type": "divider",
                    "divider": {}
                }));
            }
            Event::SoftBreak | Event::HardBreak => {
                if !in_code_block {
                    current_paragraph.push(json!({
                        "type": "text",
                        "text": {
                            "content": "\n"
                        }
                    }));
                } else {
                    code_block_content.push('\n');
                }
            }
            _ => {}
        }
    }

    flush_paragraph(&mut current_paragraph, &mut blocks);
    flush_list(&mut current_list_items, &mut blocks);

    blocks
}

fn flush_paragraph(paragraph: &mut Vec<Value>, blocks: &mut Vec<Value>) {
    if !paragraph.is_empty() {
        blocks.push(json!({
            "object": "block",
            "type": "paragraph",
            "paragraph": {
                "rich_text": paragraph.clone()
            }
        }));
        paragraph.clear();
    }
}

fn flush_list(list_items: &mut Vec<Value>, blocks: &mut Vec<Value>) {
    if !list_items.is_empty() {
        blocks.append(list_items);
        list_items.clear();
    }
}
