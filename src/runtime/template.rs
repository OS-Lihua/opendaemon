use crate::runtime::adapter::RuntimeAdapterError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TemplateValues {
    pub prompt: Option<String>,
    pub model: Option<String>,
    pub workspace: Option<String>,
    pub task_id: Option<String>,
    pub agent_id: Option<String>,
    pub directory_id: Option<String>,
}

pub struct CommandTemplate;

impl CommandTemplate {
    pub fn render_args(
        args: &[String],
        values: &TemplateValues,
    ) -> Result<Vec<String>, RuntimeAdapterError> {
        args.iter().map(|arg| Self::render(arg, values)).collect()
    }

    pub fn render(input: &str, values: &TemplateValues) -> Result<String, RuntimeAdapterError> {
        let mut rendered = String::new();
        let mut remaining = input;

        while let Some(start) = remaining.find("{{") {
            rendered.push_str(&remaining[..start]);
            let after_start = &remaining[start + 2..];
            let Some(end) = after_start.find("}}") else {
                return Err(render_error("unterminated template variable"));
            };
            let variable = after_start[..end].trim();
            rendered.push_str(resolve_variable(variable, values)?);
            remaining = &after_start[end + 2..];
        }

        rendered.push_str(remaining);
        Ok(rendered)
    }
}

fn resolve_variable<'a>(
    variable: &str,
    values: &'a TemplateValues,
) -> Result<&'a str, RuntimeAdapterError> {
    let value = match variable {
        "prompt" => values.prompt.as_deref(),
        "model" => values.model.as_deref(),
        "workspace" => values.workspace.as_deref(),
        "task_id" => values.task_id.as_deref(),
        "agent_id" => values.agent_id.as_deref(),
        "directory_id" => values.directory_id.as_deref(),
        _ => {
            return Err(render_error(format!(
                "unknown template variable: {variable}"
            )));
        }
    };

    value.ok_or_else(|| render_error(format!("missing template variable: {variable}")))
}

fn render_error(message: impl Into<String>) -> RuntimeAdapterError {
    RuntimeAdapterError::new("command_render_failed", message)
}
