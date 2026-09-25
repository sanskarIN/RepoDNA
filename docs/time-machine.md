# The Codebase Time Machine

The Time Machine shows how a repository came to be what it is: what it looked like at
points in its history, which periods it went through, which events shaped it, and a story
in which every sentence is a fact linked to commits or an interpretation labeled as such.

```sh
repodna timeline
repodna show --section time-machine,evolution
repodna analyze . --profile deep     # also reconstructs the architecture at every snapshot
```

![Time Machine view of RepoDNA's own analysis](images/time-machine.png)

It needs Git history, and it runs in the `standard`, `deep`, and `history-only` profiles.
Turn it off with `analysis.include_history = false`.

## Snapshots

A snapshot is the repository as it existed at one revision, read from Git without checking
anything out. RepoDNA chooses up to `analysis.snapshots` (12) revisions:

| Kind | Revision |
|---|---|
| Initial | The first commit. |
| Release | Commits with release tags, such as `v1.0.0`. |
| Sample | Evenly spaced commits in between. |
| Current | The analyzed revision. |

For each snapshot RepoDNA records the files, their total size, languages (by bytes, since
historical line counts would require reading every historical file), test and
documentation files, and the top-level directories. With the `deep` profile it also reads
the source files at that revision and reconstructs its **modules and module dependencies**,
so the architecture view can be compared across time. Files unchanged since the previous
snapshot are not read or parsed again.

In the web interface, move the snapshot slider to see the repository at each point: its
size, languages, test and documentation files, directories, and, with the `deep` profile,
its module graph. A growth chart plots files, size, or commits across the snapshots.

## Epochs

Epochs are contiguous periods of development, separated by long gaps or calendar years:

| Kind | Meaning |
|---|---|
| Initial | The first period of the history. |
| Active | A period whose commit rate is close to or above the repository's median. |
| Maintenance | A period whose commit rate is well below the median. |
| Current | The most recent period. |

Each epoch lists its commits, contributors, lines added and removed, and the areas it focused on.

## Events

Events are moments the history makes visible, each backed by a commit or a snapshot:

| Event | Evidence |
|---|---|
| Repository created | The first commit. |
| Module introduced, module removed | A top-level area appears or disappears. |
| Restructuring | Many files were renamed or moved in one commit. |
| Language introduced, language shift | A language appears with a meaningful share, or its share changes substantially. |
| Framework adopted, framework removed | A notable framework dependency is added or removed. |
| Tests introduced, CI adopted, containers adopted | Test files, CI configuration, or container definitions first appear. |
| Significant growth, significant reduction | The repository grows or shrinks substantially between snapshots. |
| Multi-package structure | The repository starts declaring several packages. |
| Release | A release tag. |

## The project story

The story narrates the history chronologically from the facts above. Every sentence is
either a **fact** that links to its commits and snapshots, or an **interpretation**
labeled as such (for example "Recent work concentrates on `crates/`."). It never guesses
at motives, and it says nothing about people beyond counts of contributor identities.

## Module ages and quiet areas

RepoDNA records when each module first appeared and last changed, and reports areas that
stopped changing while the rest of the repository continued (`evolution.abandoned-area`),
new modules (`evolution.new-module`), and quiet periods (`evolution.dormant-period`).

## Limits

- Snapshots see committed content only, and only the revisions that were chosen.
- Rewritten history (squashes, rebases, imports from other version control systems) shows
  the history as it is recorded now, not as it happened.
- Shallow clones have little or no history; analyze a full clone for a complete picture.
