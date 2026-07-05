use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use serde_json::{json, Value};

pub fn markdown_to_notion_blocks(markdown: &str) -> Vec<Value> {
    let parser = Parser::new(markdown);
    let mut blocks = Vec::new();
    let mut current_paragraph: Vec<Value> = Vec::new();
    let mut current_list_items: Vec<Value> = Vec::new();
    let mut in_code_block = false;
    let mut in_heading = false;
    let mut in_quote = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    // Stack of open lists: true = ordered, false = bulleted
    let mut list_stack: Vec<bool> = Vec::new();

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
                in_heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
            }
            Event::Start(Tag::Paragraph) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
            }
            Event::End(TagEnd::Paragraph) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
            }
            Event::Start(Tag::List(start)) => {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                list_stack.push(start.is_some());
            }
            Event::End(TagEnd::List(_)) => {
                flush_list(&mut current_list_items, &mut blocks);
                list_stack.pop();
            }
            Event::Start(Tag::Item) => {
                let item_type = if list_stack.last().copied().unwrap_or(false) {
                    "numbered_list_item"
                } else {
                    "bulleted_list_item"
                };
                current_list_items.push(json!({
                    "object": "block",
                    "type": item_type,
                    item_type: {
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
                in_quote = true;
            }
            Event::End(TagEnd::BlockQuote) => {
                in_quote = false;
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else if !current_list_items.is_empty() {
                    push_to_last_list_item(&mut current_list_items, json!({
                        "type": "text",
                        "text": {
                            "content": text.to_string()
                        }
                    }));
                } else if in_heading || in_quote {
                    push_to_last_block(&mut blocks, json!({
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
                    push_to_last_list_item(&mut current_list_items, rich_text_item);
                } else if in_heading || in_quote {
                    push_to_last_block(&mut blocks, rich_text_item);
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

fn push_to_last_block(blocks: &mut [Value], rich_text_item: Value) {
    if let Some(block) = blocks.last_mut() {
        let block_type = block["type"].as_str().unwrap_or("").to_string();
        if let Some(rich_text) = block[&block_type]["rich_text"].as_array_mut() {
            rich_text.push(rich_text_item);
        }
    }
}

fn push_to_last_list_item(list_items: &mut [Value], rich_text_item: Value) {
    if let Some(item) = list_items.last_mut() {
        let item_type = item["type"].as_str().unwrap_or("bulleted_list_item").to_string();
        if let Some(rich_text) = item[&item_type]["rich_text"].as_array_mut() {
            rich_text.push(rich_text_item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(block: &Value) -> String {
        let block_type = block["type"].as_str().unwrap();
        block[block_type]["rich_text"]
            .as_array()
            .unwrap()
            .iter()
            .map(|rt| rt["text"]["content"].as_str().unwrap_or(""))
            .collect()
    }

    #[test]
    fn empty_input_produces_no_blocks() {
        assert!(markdown_to_notion_blocks("").is_empty());
    }

    #[test]
    fn headings_map_to_levels() {
        let blocks = markdown_to_notion_blocks("# One\n\n## Two\n\n### Three\n\n#### Four");
        let types: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["heading_1", "heading_2", "heading_3", "heading_3"]);
        assert_eq!(text_of(&blocks[0]), "One");
        assert_eq!(text_of(&blocks[1]), "Two");
    }

    #[test]
    fn paragraph_text() {
        let blocks = markdown_to_notion_blocks("hello world");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "paragraph");
        assert_eq!(text_of(&blocks[0]), "hello world");
    }

    #[test]
    fn multiple_paragraphs_stay_separate() {
        let blocks = markdown_to_notion_blocks("first\n\nsecond");
        assert_eq!(blocks.len(), 2);
        assert_eq!(text_of(&blocks[0]), "first");
        assert_eq!(text_of(&blocks[1]), "second");
    }

    #[test]
    fn soft_break_joins_paragraph_with_newline() {
        let blocks = markdown_to_notion_blocks("line one\nline two");
        assert_eq!(blocks.len(), 1);
        assert_eq!(text_of(&blocks[0]), "line one\nline two");
    }

    #[test]
    fn bulleted_list() {
        let blocks = markdown_to_notion_blocks("- alpha\n- beta");
        assert_eq!(blocks.len(), 2);
        for block in &blocks {
            assert_eq!(block["type"], "bulleted_list_item");
        }
        assert_eq!(text_of(&blocks[0]), "alpha");
        assert_eq!(text_of(&blocks[1]), "beta");
    }

    #[test]
    fn ordered_list_becomes_numbered_items() {
        let blocks = markdown_to_notion_blocks("1. first\n2. second");
        assert_eq!(blocks.len(), 2);
        for block in &blocks {
            assert_eq!(block["type"], "numbered_list_item");
        }
        assert_eq!(text_of(&blocks[0]), "first");
        assert_eq!(text_of(&blocks[1]), "second");
    }

    #[test]
    fn inline_code_in_list_item() {
        let blocks = markdown_to_notion_blocks("- run `cargo test` now");
        assert_eq!(blocks.len(), 1);
        let rich_text = blocks[0]["bulleted_list_item"]["rich_text"].as_array().unwrap();
        assert_eq!(rich_text.len(), 3);
        assert_eq!(rich_text[1]["text"]["content"], "cargo test");
        assert_eq!(rich_text[1]["annotations"]["code"], true);
    }

    #[test]
    fn fenced_code_block_with_language() {
        let blocks = markdown_to_notion_blocks("```rust\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "code");
        assert_eq!(blocks[0]["code"]["language"], "rust");
        assert_eq!(text_of(&blocks[0]), "fn main() {}");
    }

    #[test]
    fn fenced_code_block_without_language_is_plain_text() {
        let blocks = markdown_to_notion_blocks("```\nsome text\n```");
        assert_eq!(blocks[0]["code"]["language"], "plain text");
    }

    #[test]
    fn code_block_preserves_interior_newlines() {
        let blocks = markdown_to_notion_blocks("```\nline one\nline two\n```");
        assert_eq!(text_of(&blocks[0]), "line one\nline two");
    }

    #[test]
    fn inline_code_in_paragraph() {
        let blocks = markdown_to_notion_blocks("use `foo` here");
        assert_eq!(blocks.len(), 1);
        let rich_text = blocks[0]["paragraph"]["rich_text"].as_array().unwrap();
        assert_eq!(rich_text.len(), 3);
        assert_eq!(rich_text[0]["text"]["content"], "use ");
        assert_eq!(rich_text[1]["text"]["content"], "foo");
        assert_eq!(rich_text[1]["annotations"]["code"], true);
        assert_eq!(rich_text[2]["text"]["content"], " here");
    }

    #[test]
    fn paragraph_after_heading_stays_separate() {
        let blocks = markdown_to_notion_blocks("# Title\n\nbody text");
        assert_eq!(blocks.len(), 2);
        assert_eq!(text_of(&blocks[0]), "Title");
        assert_eq!(blocks[1]["type"], "paragraph");
        assert_eq!(text_of(&blocks[1]), "body text");
    }

    #[test]
    fn inline_code_in_heading() {
        let blocks = markdown_to_notion_blocks("# Use `foo`");
        assert_eq!(blocks.len(), 1);
        let rich_text = blocks[0]["heading_1"]["rich_text"].as_array().unwrap();
        assert_eq!(rich_text.len(), 2);
        assert_eq!(rich_text[1]["text"]["content"], "foo");
        assert_eq!(rich_text[1]["annotations"]["code"], true);
    }

    #[test]
    fn blockquote() {
        let blocks = markdown_to_notion_blocks("> wisdom");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["type"], "quote");
        assert_eq!(text_of(&blocks[0]), "wisdom");
    }

    #[test]
    fn horizontal_rule_becomes_divider() {
        let blocks = markdown_to_notion_blocks("above\n\n---\n\nbelow");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[1]["type"], "divider");
    }

    #[test]
    fn mixed_document() {
        let markdown = "# Title\n\nIntro paragraph.\n\n- item\n\n```sh\necho hi\n```\n";
        let blocks = markdown_to_notion_blocks(markdown);
        let types: Vec<&str> = blocks.iter().map(|b| b["type"].as_str().unwrap()).collect();
        assert_eq!(types, vec!["heading_1", "paragraph", "bulleted_list_item", "code"]);
    }

    #[test]
    fn unicode_text_passes_through() {
        let blocks = markdown_to_notion_blocks("café — “quoted”");
        assert_eq!(text_of(&blocks[0]), "café — “quoted”");
    }
}
