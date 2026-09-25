//! `repodna explain`: optional AI explanations grounded in the analysis.

use std::path::Path;

use repodna_ai::AiTask;
use repodna_ai::render::{markdown, text};
use repodna_app::explain::provider;
use repodna_app::{AppError, ExplainRequest, explain, plan_explanation};

use super::{Ctx, print, to_json};
use crate::cli::{AboutArg, ExplainCmd, ExplainFormat};

fn task(cmd: &ExplainCmd) -> Result<AiTask, AppError> {
    let (id, subject) = if let Some(path) = &cmd.module {
        ("module", Some(path.as_str()))
    } else if let Some(path) = &cmd.hotspot {
        ("hotspot", Some(path.as_str()))
    } else if let Some(question) = &cmd.ask {
        ("question", Some(question.as_str()))
    } else {
        let id = match cmd.about {
            AboutArg::Repository => "repository",
            AboutArg::Architecture => "architecture",
            AboutArg::History => "history",
            AboutArg::Dependencies => "dependencies",
            AboutArg::Onboarding => "onboarding",
        };
        (id, None)
    };
    AiTask::parse(id, subject).map_err(AppError::usage)
}

/// Runs `repodna explain`.
pub fn run(ctx: &Ctx, cmd: &ExplainCmd) -> Result<(), AppError> {
    let task = task(cmd)?;
    let config = ctx.config_for(&cmd.target, &cmd.analysis)?;
    if !cmd.dry_run {
        // Fail before analyzing when no usable provider is configured.
        provider(&config, cmd.allow_remote_ai)?;
    }
    let loaded = ctx.load(&cmd.target, &cmd.analysis)?;
    let checkout = Path::new(&cmd.target);
    let request = ExplainRequest {
        task,
        allow_remote: cmd.allow_remote_ai,
        use_cache: !cmd.fresh,
        checkout: checkout.is_dir().then_some(checkout),
    };

    if cmd.dry_run {
        let plan = plan_explanation(&config, &loaded.dna, &request)?;
        if cmd.format == ExplainFormat::Json {
            return ctx.emit(cmd.output.as_ref(), &to_json(&plan)?, cmd.force);
        }
        let destination = match provider(&config, cmd.allow_remote_ai) {
            Ok(provider) => format!(
                "{} / {} at {} ({})",
                provider.id(),
                provider.model(),
                provider.destination(),
                if provider.remote() {
                    "leaves this machine"
                } else {
                    "stays on this machine"
                }
            ),
            Err(error) => format!("not available: {}", error.message),
        };
        let cost = plan.estimated_max_cost.map_or_else(
            || {
                "not estimated (set ai.input_cost_per_million and ai.output_cost_per_million)"
                    .to_owned()
            },
            |cost| format!("at most {cost:.4} in the currency of your configured prices"),
        );
        let out = format!(
            "Dry run: nothing was sent.\n\nProvider: {destination}\nEvidence: {} items ({} left out to fit the budget); source excerpts: {}\nEstimated input: {} tokens; output limit: {} tokens\nCost: {cost}\n\n--- system prompt ---\n{}\n\n--- user message ---\n{}\n",
            plan.context.items.len(),
            plan.context.omitted,
            if plan.context.excerpts {
                "yes, redacted"
            } else {
                "no"
            },
            plan.estimated_input_tokens,
            plan.max_output_tokens,
            plan.prompt.system,
            plan.prompt.user
        );
        return ctx.emit(cmd.output.as_ref(), &out, cmd.force);
    }

    ctx.note(&ctx.err.dim("Asking the configured AI provider; its answer is checked against the evidence it was given."));
    let outcome = explain(&ctx.paths, &config, &loaded.dna, &request, &ctx.cancel)?;
    if outcome.cached {
        ctx.note(
            &ctx.err
                .dim("Reusing a cached explanation (pass --fresh to ask again)."),
        );
    }
    let rendered = match cmd.format {
        ExplainFormat::Text => text(&outcome.explanation),
        ExplainFormat::Markdown => markdown(&outcome.explanation),
        ExplainFormat::Json => to_json(&outcome.explanation)?,
    };
    match &cmd.output {
        Some(_) => ctx.emit(cmd.output.as_ref(), &rendered, cmd.force),
        None => print(&rendered),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use clap::Parser;

    fn parse(args: &[&str]) -> ExplainCmd {
        let mut all = vec!["repodna", "explain"];
        all.extend_from_slice(args);
        match Cli::try_parse_from(all).unwrap().command {
            Command::Explain(cmd) => cmd,
            _ => unreachable!(),
        }
    }

    #[test]
    fn maps_flags_to_tasks() {
        assert_eq!(task(&parse(&[])).unwrap(), AiTask::Repository);
        assert_eq!(
            task(&parse(&["--about", "history"])).unwrap(),
            AiTask::History
        );
        assert_eq!(
            task(&parse(&["--module", "src/net/"])).unwrap(),
            AiTask::Module("src/net".into())
        );
        assert_eq!(
            task(&parse(&["--ask", "Where is the parser?"])).unwrap(),
            AiTask::Question("Where is the parser?".into())
        );
        assert!(task(&parse(&["--ask", "  "])).is_err());
    }
}
