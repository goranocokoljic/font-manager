//! The MCP prompts (E27.5): ready-made requests that tell the client which
//! tools to call, in which order.
//!
//! `choose_fonts_for_brief` (brief, languages) leads to `pick_fonts_for_brief`
//! and `render_preview`; `check_language_coverage` (text, optional font)
//! leads to `check_coverage`. A prompt is one user message of plain text;
//! it calls nothing itself. Prompt arguments are strings (MCP): `languages`
//! is a comma-separated list of preset codes.

use rmcp::ErrorData;
use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};
use serde_json::Value;

use crate::tools::{CHECK_COVERAGE, RENDER_PREVIEW, SEARCH_FONTS};

/// Choose fonts from the library for a design brief.
pub const CHOOSE_FONTS_FOR_BRIEF: &str = "choose_fonts_for_brief";
/// Check which fonts cover a text, or whether one font does.
pub const CHECK_LANGUAGE_COVERAGE: &str = "check_language_coverage";

/// The tool that picks faces for a brief (E27.4).
pub const PICK_FONTS_FOR_BRIEF: &str = "pick_fonts_for_brief";

/// Every prompt, in the order `prompts/list` gives them.
pub const NAMES: [&str; 2] = [CHOOSE_FONTS_FOR_BRIEF, CHECK_LANGUAGE_COVERAGE];

/// The prompts with their arguments.
pub fn list() -> Vec<Prompt> {
    let arg = |name: &str, description: &str, required: bool| {
        PromptArgument::new(name)
            .with_description(description)
            .with_required(required)
    };
    vec![
        Prompt::new(
            CHOOSE_FONTS_FOR_BRIEF,
            Some("Choose fonts from the library for a design brief, and look at them."),
            Some(vec![
                arg(
                    "brief",
                    "The design brief: what the fonts are for, the tone, the medium.",
                    true,
                ),
                arg(
                    "languages",
                    "Language preset codes the fonts must cover, comma-separated (sr-Cyrl, pl).",
                    false,
                ),
            ]),
        )
        .with_title("Choose fonts for a brief"),
        Prompt::new(
            CHECK_LANGUAGE_COVERAGE,
            Some("Check which library fonts cover every character of a text."),
            Some(vec![
                arg("text", "The text every character of which must be covered.", true),
                arg(
                    "font",
                    "One font to check: a face id or a name. Without it, find the fonts that cover the text.",
                    false,
                ),
            ]),
        )
        .with_title("Check language coverage"),
    ]
}

/// The messages of prompt `name` for `arguments`. An unknown prompt or a
/// missing required argument is an invalid-params error.
pub fn get(name: &str, arguments: Option<JsonObject>) -> Result<GetPromptResult, ErrorData> {
    let arguments = arguments.unwrap_or_default();
    let (description, text) = match name {
        CHOOSE_FONTS_FOR_BRIEF => {
            let brief = required(name, &arguments, "brief")?;
            let languages = languages(optional(&arguments, "languages").as_deref());
            ("Choose fonts for a brief", choose(&brief, &languages))
        }
        CHECK_LANGUAGE_COVERAGE => {
            let text = required(name, &arguments, "text")?;
            let font = optional(&arguments, "font");
            ("Check language coverage", coverage(&text, font.as_deref()))
        }
        _ => {
            return Err(ErrorData::invalid_params(
                format!("There is no prompt named {name}."),
                None,
            ));
        }
    };
    Ok(
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
            .with_description(description),
    )
}

fn choose(brief: &str, languages: &[String]) -> String {
    let (cover, args) = if languages.is_empty() {
        (String::new(), "the brief".to_owned())
    } else {
        let list = languages.join(", ");
        let json = serde_json::to_string(languages).unwrap_or_default();
        (
            format!("\n\nThe fonts must cover these languages: {list}."),
            format!("the brief and `languages` set to {json}"),
        )
    };
    format!(
        "Choose fonts from my Font Manager library for this brief:\n\n{brief}{cover}\n\n\
         1. Call `{PICK_FONTS_FOR_BRIEF}` with {args}. It answers with candidate faces \
         from the library, with their face ids.\n\
         2. Call `{RENDER_PREVIEW}` for each candidate you consider, with a short line \
         of text that fits the brief, and look at the picture.\n\
         3. Answer with your picks: family, style and face id, and one sentence each on \
         why it fits the brief."
    )
}

fn coverage(text: &str, font: Option<&str>) -> String {
    let quoted = serde_json::to_string(text).unwrap_or_default();
    match font {
        Some(font) => format!(
            "Check whether the font {font} in my Font Manager library covers every \
             character of this text:\n\n{text}\n\n\
             1. If {font} is not a face id, call `{SEARCH_FONTS}` with `query` set to \
             the font's name to find its face id.\n\
             2. Call `{CHECK_COVERAGE}` with the face id and `text` set to {quoted}.\n\
             3. Tell me whether it covers the text, and list the characters it misses."
        ),
        None => format!(
            "Find the fonts in my Font Manager library that cover every character of \
             this text:\n\n{text}\n\n\
             1. Call `{SEARCH_FONTS}` with `text` set to {quoted} to find candidate faces.\n\
             2. Call `{CHECK_COVERAGE}` with each candidate's face id and `text` set to \
             {quoted} to confirm it and see what it misses.\n\
             3. Answer with the fonts that cover the text, and for the near misses the \
             characters they lack."
        ),
    }
}

/// The trimmed, non-empty codes of a comma-separated list.
fn languages(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|code| !code.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A non-empty string argument, or `None`.
fn optional(arguments: &JsonObject, key: &str) -> Option<String> {
    match arguments.get(key)? {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn required(name: &str, arguments: &JsonObject, key: &str) -> Result<String, ErrorData> {
    optional(arguments, key).ok_or_else(|| {
        ErrorData::invalid_params(format!("The prompt {name} needs the argument {key}."), None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_are_a_comma_separated_list() {
        assert_eq!(languages(Some(" sr-Cyrl, pl ,,")), ["sr-Cyrl", "pl"]);
        assert!(languages(None).is_empty());
        assert!(languages(Some(" , ")).is_empty());
    }

    #[test]
    fn a_blank_argument_counts_as_missing() {
        let mut args = JsonObject::new();
        args.insert("brief".into(), Value::String("  ".into()));
        assert!(required(CHOOSE_FONTS_FOR_BRIEF, &args, "brief").is_err());
        args.insert("brief".into(), Value::String(" A bakery ".into()));
        assert_eq!(
            required(CHOOSE_FONTS_FOR_BRIEF, &args, "brief")
                .ok()
                .as_deref(),
            Some("A bakery")
        );
    }
}
