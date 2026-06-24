//! Helpers shared by the Noema CLI binary.

use thiserror::Error;

/// Errors produced while collecting a prompt for the CLI.
#[derive(Debug, Error)]
pub enum PromptError {
    /// No prompt text was provided by arguments or standard input.
    #[error("prompt cannot be empty")]
    Empty,
}

/// Collect a prompt from trailing arguments or stdin.
///
/// # Errors
///
/// Returns [`PromptError::Empty`] when both sources are empty after trimming.
pub fn collect_prompt(args: &[String], stdin: &str) -> Result<String, PromptError> {
    let prompt = if args.is_empty() {
        stdin.trim().to_string()
    } else {
        args.join(" ").trim().to_string()
    };

    if prompt.is_empty() {
        Err(PromptError::Empty)
    } else {
        Ok(prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_prompt_from_args() {
        let prompt = collect_prompt(&["hello".to_string(), "world".to_string()], "ignored stdin")
            .expect("prompt");

        assert_eq!(prompt, "hello world");
    }

    #[test]
    fn collects_prompt_from_stdin_when_args_are_empty() {
        let prompt = collect_prompt(&[], " hello from stdin\n").expect("prompt");

        assert_eq!(prompt, "hello from stdin");
    }

    #[test]
    fn empty_prompt_is_an_error() {
        let error = collect_prompt(&[], " \n\t ").unwrap_err();

        assert!(matches!(error, PromptError::Empty));
    }
}
