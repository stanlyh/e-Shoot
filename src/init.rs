use crate::config::schema::{
    ApiConfig, ComponentDefinition, ComponentType, Config, JobDefinition, LoggingConfig,
    ParameterDefinition, ParameterType, RecipientGroup, TemplateDefinition,
};
use console::style;
use dialoguer::{Confirm, Input, Password, Select};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub fn run(output: Option<PathBuf>) -> anyhow::Result<()> {
    let out_path = output.unwrap_or_else(|| PathBuf::from("config.toml"));

    print_header();

    if out_path.exists() {
        let overwrite = Confirm::new()
            .with_prompt(format!(
                "{} already exists. Overwrite?",
                out_path.display()
            ))
            .default(false)
            .interact()?;
        if !overwrite {
            println!("Aborted.");
            return Ok(());
        }
    }

    let api = ask_api()?;
    let recipients = ask_recipients()?;
    let templates = ask_templates()?;
    let jobs = ask_jobs(&recipients, &templates)?;

    let config = Config {
        api,
        logging: LoggingConfig::default(),
        recipients,
        templates,
        jobs,
    };

    write_config(&config, &out_path)?;

    println!();
    println!(
        "{} Config written to {}",
        style("✔").green().bold(),
        style(out_path.display()).cyan()
    );
    println!(
        "  Run {} to validate.",
        style("e-shoot check").yellow()
    );

    Ok(())
}

// ── Sections ─────────────────────────────────────────────────────────────────

fn ask_api() -> anyhow::Result<ApiConfig> {
    section("API Configuration");

    let base_url = Input::<String>::new()
        .with_prompt("Base URL")
        .default("https://graph.facebook.com/v19.0".into())
        .interact_text()?;

    let token = Password::new()
        .with_prompt("API token (or leave blank to use E_SHOOT_API_TOKEN env var)")
        .allow_empty_password(true)
        .interact()?;

    let phone_number_id = Input::<String>::new()
        .with_prompt("Phone number ID")
        .interact_text()?;

    Ok(ApiConfig {
        base_url,
        token: if token.is_empty() {
            "${E_SHOOT_API_TOKEN}".to_string()
        } else {
            token
        },
        phone_number_id,
        connect_timeout_secs: 5,
        request_timeout_secs: 30,
        max_retries: 3,
        retry_backoff_secs: 2,
    })
}

fn ask_recipients() -> anyhow::Result<Vec<RecipientGroup>> {
    section("Recipient Groups");

    let mut groups = Vec::new();

    loop {
        let prompt = if groups.is_empty() {
            "Add a recipient group?".to_string()
        } else {
            "Add another recipient group?".to_string()
        };

        if !Confirm::new()
            .with_prompt(prompt)
            .default(groups.is_empty())
            .interact()?
        {
            break;
        }

        let name = Input::<String>::new()
            .with_prompt("  Group name")
            .interact_text()?;

        let mut numbers = Vec::new();
        loop {
            let n: String = Input::new()
                .with_prompt("  Phone number (E.164 format, empty to finish)")
                .allow_empty(true)
                .interact_text()?;
            if n.trim().is_empty() {
                break;
            }
            numbers.push(n.trim().to_string());
        }

        if numbers.is_empty() {
            println!("  {} Skipping empty group.", style("!").yellow());
            continue;
        }

        println!(
            "  {} Group '{}' with {} number(s).",
            style("✔").green(),
            name,
            numbers.len()
        );
        groups.push(RecipientGroup { name, numbers });
    }

    Ok(groups)
}

fn ask_templates() -> anyhow::Result<Vec<TemplateDefinition>> {
    section("Message Templates");
    println!(
        "  {}",
        style("Use {{1}}, {{2}}, … for variable placeholders.").dim()
    );

    let mut templates = Vec::new();

    loop {
        let prompt = if templates.is_empty() {
            "Add a template?".to_string()
        } else {
            "Add another template?".to_string()
        };

        if !Confirm::new()
            .with_prompt(prompt)
            .default(templates.is_empty())
            .interact()?
        {
            break;
        }

        let name = Input::<String>::new()
            .with_prompt("  Template name (must match one approved in WhatsApp Business)")
            .interact_text()?;

        let language = Input::<String>::new()
            .with_prompt("  Language code")
            .default("es".into())
            .interact_text()?;

        let body = Input::<String>::new()
            .with_prompt("  Body text")
            .interact_text()?;

        let n_vars = count_variables(&body);
        if n_vars > 0 {
            println!(
                "  {} Detected {} variable(s): {}",
                style("ℹ").blue(),
                n_vars,
                (1..=n_vars)
                    .map(|i| format!("{{{{{i}}}}}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        println!(
            "  {} Template '{}'.",
            style("✔").green(),
            name
        );

        templates.push(TemplateDefinition {
            name,
            language,
            components: vec![ComponentDefinition {
                kind: ComponentType::Body,
                parameters: vec![ParameterDefinition {
                    kind: ParameterType::Text,
                    value: Some(body),
                }],
            }],
        });
    }

    Ok(templates)
}

fn ask_jobs(
    recipients: &[RecipientGroup],
    templates: &[TemplateDefinition],
) -> anyhow::Result<Vec<JobDefinition>> {
    section("Jobs");

    if templates.is_empty() || recipients.is_empty() {
        println!(
            "  {} No templates or recipient groups defined — skipping jobs.",
            style("!").yellow()
        );
        return Ok(Vec::new());
    }

    let mut jobs = Vec::new();

    loop {
        let prompt = if jobs.is_empty() {
            "Add a job?".to_string()
        } else {
            "Add another job?".to_string()
        };

        if !Confirm::new()
            .with_prompt(prompt)
            .default(jobs.is_empty())
            .interact()?
        {
            break;
        }

        let id = Input::<String>::new()
            .with_prompt("  Job ID (unique, no spaces)")
            .interact_text()?;

        let description: String = Input::new()
            .with_prompt("  Description (optional)")
            .allow_empty(true)
            .interact_text()?;

        // Schedule type
        let schedule_types = ["Cron (recurring)", "Once (specific ISO 8601 date-time)"];
        let stype = Select::new()
            .with_prompt("  Schedule type")
            .items(&schedule_types)
            .default(0)
            .interact()?;

        let (schedule, schedule_once) = if stype == 0 {
            let expr = ask_cron_expression()?;
            (Some(expr), None)
        } else {
            let dt = ask_once_datetime()?;
            (None, Some(dt))
        };

        // Template
        let tmpl_names: Vec<&str> = templates.iter().map(|t| t.name.as_str()).collect();
        let tmpl_idx = Select::new()
            .with_prompt("  Template")
            .items(&tmpl_names)
            .default(0)
            .interact()?;
        let chosen_template = &templates[tmpl_idx];

        // Recipient group
        let grp_names: Vec<&str> = recipients.iter().map(|r| r.name.as_str()).collect();
        let grp_idx = Select::new()
            .with_prompt("  Recipient group")
            .items(&grp_names)
            .default(0)
            .interact()?;
        let chosen_group = &recipients[grp_idx];

        // Variables
        let n_vars = template_variable_count(chosen_template);
        let variables = if n_vars > 0 {
            ask_variables(chosen_group, n_vars)?
        } else {
            HashMap::new()
        };

        println!("  {} Job '{}'.", style("✔").green(), id);

        jobs.push(JobDefinition {
            id,
            enabled: true,
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            schedule,
            schedule_once,
            timezone: None,
            template: chosen_template.name.clone(),
            recipients: chosen_group.name.clone(),
            variables,
        });
    }

    Ok(jobs)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn ask_cron_expression() -> anyhow::Result<String> {
    println!(
        "  {}",
        style("  Format: sec min hour day-of-month month day-of-week").dim()
    );
    println!("  {}", style("  Examples:").dim());
    println!("  {}", style("    Every day at 9am UTC  →  0 0 9 * * *").dim());
    println!("  {}", style("    Every Monday 9am      →  0 0 9 * * MON").dim());
    println!("  {}", style("    Every hour            →  0 0 * * * *").dim());

    loop {
        let expr = Input::<String>::new()
            .with_prompt("  Cron expression")
            .default("0 0 9 * * MON".into())
            .interact_text()?;

        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() == 6 {
            return Ok(expr);
        }
        println!(
            "  {} Expected 6 fields, got {}. Try again.",
            style("✖").red(),
            parts.len()
        );
    }
}

fn ask_once_datetime() -> anyhow::Result<chrono::DateTime<chrono::Utc>> {
    println!(
        "  {}",
        style("  Format: 2026-06-01T10:00:00Z").dim()
    );
    loop {
        let raw = Input::<String>::new()
            .with_prompt("  Date-time (ISO 8601 UTC)")
            .interact_text()?;

        match raw.parse::<chrono::DateTime<chrono::Utc>>() {
            Ok(dt) => return Ok(dt),
            Err(_) => println!(
                "  {} Invalid date-time. Use format 2026-06-01T10:00:00Z",
                style("✖").red()
            ),
        }
    }
}

fn ask_variables(
    group: &RecipientGroup,
    n_vars: usize,
) -> anyhow::Result<HashMap<String, Vec<String>>> {
    println!(
        "  {} Enter variable values for each recipient ({} variable(s) per recipient):",
        style("ℹ").blue(),
        n_vars
    );

    let mut map = HashMap::new();

    for number in &group.numbers {
        println!("    {}", style(number).cyan());
        let mut vars = Vec::new();
        for i in 1..=n_vars {
            let val = Input::<String>::new()
                .with_prompt(format!("      {{{{ {i} }}}}"))
                .interact_text()?;
            vars.push(val);
        }
        map.insert(number.clone(), vars);
    }

    Ok(map)
}

fn write_config(config: &Config, path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let content = toml::to_string_pretty(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

fn section(title: &str) {
    println!();
    println!("{}", style(format!("── {title} ──")).bold().cyan());
}

fn print_header() {
    println!();
    println!("{}", style("e-shoot init").bold().green());
    println!(
        "{}",
        style("Generates a config.toml by answering a few questions.").dim()
    );
}

/// Count highest {{N}} index found anywhere in a template's parameter values.
fn template_variable_count(template: &TemplateDefinition) -> usize {
    let mut max = 0usize;
    for comp in &template.components {
        for param in &comp.parameters {
            if let Some(val) = &param.value {
                max = max.max(count_variables(val));
            }
        }
    }
    max
}

/// Return the highest {{N}} index found in `text` (1-indexed → count).
fn count_variables(text: &str) -> usize {
    let mut max = 0usize;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next();
            let digits: String = chars
                .by_ref()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            if let Ok(n) = digits.parse::<usize>() {
                max = max.max(n);
            }
        }
    }
    max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_variables_correctly() {
        assert_eq!(count_variables("Hola {{1}}, descuento {{2}}%"), 2);
        assert_eq!(count_variables("Sin variables"), 0);
        assert_eq!(count_variables("Solo {{3}}"), 3);
        assert_eq!(count_variables("{{1}} y {{1}} otra vez"), 1);
    }
}
