# RepoDNA Native Model — Development Plan

Status: **offline extractive prototype**, not a trained language model.

## Goal

Make RepoDNA explanations independent of cloud services, API keys, model runtimes,
and mandatory network access while retaining measurable, cited evidence.

## Current prototype (`provider = "native"`)

- Pure Rust, in-process evidence digest; no runtime, GPU, downloads, or model weights.
- Reuses bounded context selection, citation validation, cancellation and caching.
- Converts up to twelve selected facts into cited statements without inference.
- Does not execute repository commands or reproduce source excerpts.
- Clearly declares that it is not a trained generative model.
- Remains opt-in through user configuration; repository configuration cannot enable it.

## Phase 1 — Trainable evidence ranking

Create an original or appropriately licensed annotation dataset for relevance across
repository, architecture, dependencies, history, module, hotspot, and onboarding tasks.
Split training/validation/test by **repository**, avoiding commit or snippet leakage.
Implement a CPU-capable native ranker, a reproducible training/export pipeline,
and holdout tests against the current deterministic context order.

## Phase 2 — Learned language generation

Select a feasible model architecture **after** memory and latency benchmarks.
Train or fine-tune only with data whose licensing permits the intended use.
Use locally stored, versioned and verified model weights; never download
weights silently. Keep CPU inference and memory limits as release requirements.
Preserve structured citations, add semantic claim verification and abstention,
and fall back to the current extractive mode on unsupported hardware.

## Evaluation gates

Measure grounded claim precision with manual adjudication (a valid citation ID
does not prove a true claim), citation coverage, invalid references, abstention,
task relevance, prompt-injection resistance, privacy, cancellation, CPU latency,
peak RAM, installation size, and accuracy across unrelated codebases.

Do not label a ranking baseline as a generative LLM or report unmeasured accuracy.

## Try the keyless prototype

Put this in the *user* config discovered with `repodna config path`:

```toml
[ai]
provider = "native"
```

```sh
repodna explain --about repository
repodna explain --about dependencies --format markdown
repodna explain --dry-run
```

Trained neural checkpoints and generative text inference are **not included yet**.
