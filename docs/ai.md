# AI explanations

RepoDNA's analysis is deterministic and never needs AI. `repodna explain` optionally asks
a language model to turn the analysis into prose: a first-look explanation of a
repository, its architecture, its history, its dependencies, one module, one hotspot, or
an answer to a question. It is **off until you configure a provider**, and it only ever
sends a small, numbered set of facts from the analysis, never the repository itself.

## How it works

1. RepoDNA selects evidence items from the analysis artifact that fit the task: facts,
   metrics, modules, commands, files, packages, and findings. Each gets an identifier
   (`E1`, `E2`, ...). The selection fits within `ai.max_context_tokens` (6000 by default).
2. The prompt tells the model to base every statement on those items and cite them, to
   mark statements that go beyond the evidence as inferences, and to treat all text from
   the repository (file names, descriptions, commit subjects) as data, never as
   instructions.
3. The model answers with a JSON object: a summary, up to twelve points with their cited
   evidence, a confidence level, and limitations.
4. RepoDNA checks the answer against the evidence it sent. Citations of evidence that was
   not provided are removed, statements without valid citations are labeled *not
   supported by cited evidence*, and statements the model marked as inferences are labeled
   as such. If most statements are unsupported, the answer says so.

The model has no tools: it cannot run commands, read files, or reach the network through
RepoDNA. Contributor identities are left out of the evidence. Source code is not sent
unless you set `include_source_excerpts = true`, and even then only short excerpts, with
anything that looks like a secret redacted.

## See exactly what would be sent

```sh
repodna explain --dry-run
```

This prints the provider, the number of evidence items, the estimated input size, the
output limit, a cost estimate when you have configured prices, and the full system prompt
and user message. Nothing is sent.

## Providers

Configure a provider in your **user configuration** (`repodna config path` shows where it
is). A repository's own `repodna.toml` can never configure AI.

### A local program (`command`)

Any program that reads a prompt on standard input and writes the answer to standard
output, such as [Ollama](https://ollama.com):

```toml
[ai]
provider = "command"
command = ["ollama", "run", "llama3.2"]
```

The program runs without a shell, with a time limit (`timeout_seconds`, default 300).
RepoDNA treats it as local; what the program itself does with the prompt is up to the
program.

### An OpenAI-compatible server (`openai-compatible`)

Local runtimes such as Ollama, llama.cpp's server, LM Studio, and vLLM, or hosted services
that implement the OpenAI chat completions API:

```toml
[ai]
provider = "openai-compatible"
endpoint = "http://127.0.0.1:11434/v1"
model = "llama3.2"
# api_key_env = "MY_PROVIDER_KEY"   # only for services that need a key
```

`endpoint` and `model` are required. Plain `http://` is allowed only for localhost; any
other endpoint must use `https://`.

### The Anthropic API (`anthropic`)

```toml
[privacy]
remote_ai = true

[ai]
provider = "anthropic"
model = "MODEL_ID"                 # a model your account can use
api_key_env = "ANTHROPIC_API_KEY"
```

`model` is required; use an identifier from Anthropic's list of models. The key is read
from the environment variable you name; keys are never stored in files. Without
`api_key_env`, RepoDNA reads `ANTHROPIC_API_KEY`, and only when `endpoint` is Anthropic's
own; a gateway or proxy you set as `endpoint` gets a key only from a variable you name.

RepoDNA leaves thinking and effort at the model's defaults. On models that think before
answering, the output limit (16000 tokens unless you set `max_output_tokens`) also covers
the thinking, and an answer can take a few minutes.

Some models decline requests that their safety checks flag, which can happen with
security-related evidence. RepoDNA then reports the reason the API gives; another model,
or a narrower request such as `--module`, may be answered.

## Remote providers need your consent

A provider whose endpoint is not on this machine (not `localhost`, `127.0.0.1`, or `::1`)
is refused unless you allow remote AI, either permanently with `privacy.remote_ai = true`
in your user configuration or for one run with `--allow-remote-ai`:

```sh
repodna explain --allow-remote-ai
```

## Examples

```sh
repodna explain                                  # first-look explanation of the current repository
repodna explain ~/src/project --about architecture
repodna explain --about history
repodna explain --module crates/repodna-engine
repodna explain --hotspot src/server.rs
repodna explain --ask "How are plugins loaded?"
repodna explain --format markdown -o EXPLANATION.md
repodna explain --format json                    # with provenance: provider, model, revision, cited evidence, token use
```

Explanations are cached: asking the same model, at the same endpoint or through the same
command, the same question with the same evidence reuses the earlier answer instead of
sending a request. `--fresh` asks the
provider again, and `repodna cache clear` removes cached explanations.

## Costs

RepoDNA does not know what your provider charges. To see estimates in `--dry-run` output,
set the prices yourself:

```toml
[ai]
input_cost_per_million = 3.0
output_cost_per_million = 15.0
```

`max_context_tokens` limits what is sent, and `max_output_tokens` limits what the model may
generate (16000 by default for `anthropic`, 2000 for the other providers).

## Limits

- Explanations are only as good as the analysis: they describe what RepoDNA measured, not
  what the code does at run time.
- A model can still misread the evidence. RepoDNA labels unsupported statements but cannot
  prove that a supported statement is correct; check the cited evidence.
- AI features are optional. Every other command, report, and view works without them.
