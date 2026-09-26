import { WEEKDAYS, date, percent, span, thousands } from "@repodna/visualization";
import { BarList } from "../charts/BarList";
import { Columns } from "../charts/Columns";
import { Heatmap } from "../charts/Heatmap";
import { Note, Omittable, PageHeader, Panel, SectionStatus, Tile } from "../components/common";
import { DataTable } from "../components/DataTable";
import { MIN_MONTHS_FOR_MONTHLY_CHART, continuousDays } from "../lib/activity";
import { contributorNamer, count } from "../lib/names";
import { useDataset } from "../state";

const ACTIVITY: Record<string, string> = {
  "very-active": "Very active",
  active: "Active",
  moderate: "Moderately active",
  low: "Low activity",
  dormant: "Dormant",
  none: "No activity",
};

export function History() {
  const { dna } = useDataset();
  const git = dna.git;

  if (git.commitCount === 0) {
    return (
      <>
        <PageHeader title="History" />
        <SectionStatus
          status={git.status === "analyzed" ? "unavailable" : git.status}
          notes={git.notes}
        />
        <p className="muted">
          {dna.identity.isGitRepository
            ? "This analysis has no commit history."
            : "This was not analyzed as a Git repository, so there is no history. Analyze a Git checkout or a Git URL to see how it evolved."}
        </p>
      </>
    );
  }

  const busiest = git.weekdayHour.map((hours, day) => {
    let max = 0;
    let at = 0;
    hours.forEach((count, hour) => {
      if (count > max) {
        max = count;
        at = hour;
      }
    });
    return {
      day: WEEKDAYS[day] ?? String(day),
      total: hours.reduce((sum, count) => sum + count, 0),
      busiest: max > 0 ? `${String(at).padStart(2, "0")}:00` : "–",
    };
  });
  const contributors = [...git.contributors].sort((a, b) => b.commits - a.commits);
  const author = contributorNamer(dna);
  // One or two monthly columns say little about a young repository; show its days instead.
  const days =
    git.timeline.length < MIN_MONTHS_FOR_MONTHLY_CHART ? continuousDays(git.dailyActivity) : null;

  return (
    <>
      <PageHeader title="History">
        What Git records about how this repository changed:{" "}
        {ACTIVITY[git.activity.level]?.toLowerCase() ?? git.activity.level}.{" "}
        {git.activity.description}
      </PageHeader>
      <SectionStatus status={git.status} notes={git.notes} />
      {git.shallow ? (
        <Note caution>
          This is a shallow clone, so older history is missing and early dates may be wrong.
        </Note>
      ) : null}
      {git.historyTruncated ? (
        <Note caution>
          History was truncated to the configured commit limit; counts cover the analyzed commits
          only.
        </Note>
      ) : null}
      <div className="tiles">
        <Tile label="Commits" value={thousands(git.commitCount)} />
        <Tile label="Contributors" value={thousands(git.ownership.contributors)} />
        <Tile label="First commit" value={date(git.firstCommit?.timestamp)} />
        <Tile
          label="Last commit"
          value={date(git.lastCommit?.timestamp)}
          note={
            git.activity.daysSinceLastCommit != null
              ? `${span(git.activity.daysSinceLastCommit)} before the analysis`
              : undefined
          }
        />
        <Tile
          label="Commits in the last 90 days"
          value={thousands(git.activity.commitsLast90Days)}
          note={`${thousands(git.activity.commitsLast365Days)} in the last year`}
        />
        <Tile
          label="Releases"
          value={thousands(git.releases.length)}
          note={count(git.tags.length, "tag", "tags")}
        />
      </div>

      {days ? (
        <Panel
          title="Commits over time"
          description="Each column is one day (UTC); hover or focus a column for lines changed."
          table={
            <DataTable
              rows={days}
              rowKey={(d) => d.date}
              columns={[
                { key: "date", header: "Day", cell: (d) => d.date, sort: (d) => d.date },
                {
                  key: "commits",
                  header: "Commits",
                  cell: (d) => thousands(d.commits),
                  sort: (d) => d.commits,
                  numeric: true,
                },
                {
                  key: "churn",
                  header: "Lines changed",
                  cell: (d) => thousands(d.churn),
                  sort: (d) => d.churn,
                  numeric: true,
                },
              ]}
            />
          }
        >
          <Columns
            label="Commits per day"
            points={days.map((d) => ({
              label: d.date,
              value: d.commits,
              details: `${thousands(d.churn)} lines added or removed`,
            }))}
            unit={[" commit", " commits"]}
            labelWidth={76}
          />
        </Panel>
      ) : (
        <Panel
          title="Commits over time"
          description="Each column is one month; hover or focus a column for authors and lines changed."
          table={
            <DataTable
              rows={git.timeline}
              rowKey={(b) => b.start}
              columns={[
                { key: "period", header: "Period", cell: (b) => b.period, sort: (b) => b.start },
                {
                  key: "commits",
                  header: "Commits",
                  cell: (b) => thousands(b.commits),
                  sort: (b) => b.commits,
                  numeric: true,
                },
                {
                  key: "authors",
                  header: "Authors",
                  cell: (b) => thousands(b.authors),
                  sort: (b) => b.authors,
                  numeric: true,
                },
                {
                  key: "insertions",
                  header: "Lines added",
                  cell: (b) => thousands(b.insertions),
                  sort: (b) => b.insertions,
                  numeric: true,
                },
                {
                  key: "deletions",
                  header: "Lines removed",
                  cell: (b) => thousands(b.deletions),
                  sort: (b) => b.deletions,
                  numeric: true,
                },
              ]}
            />
          }
        >
          <Columns
            label="Commits per period"
            points={git.timeline.map((b) => ({
              label: b.period,
              value: b.commits,
              details: `${count(b.authors, "author", "authors")} · +${thousands(b.insertions)} −${thousands(b.deletions)}`,
            }))}
            unit={[" commit", " commits"]}
          />
        </Panel>
      )}

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel
          title="When commits happen"
          description="By weekday and hour in each commit's own time zone."
          table={
            <DataTable
              rows={busiest}
              rowKey={(row) => row.day}
              columns={[
                { key: "day", header: "Day", cell: (row) => row.day },
                {
                  key: "total",
                  header: "Commits",
                  cell: (row) => thousands(row.total),
                  sort: (row) => row.total,
                  numeric: true,
                },
                { key: "busiest", header: "Busiest hour", cell: (row) => row.busiest },
              ]}
            />
          }
        >
          <Heatmap grid={git.weekdayHour} label="Commits by weekday and hour" />
        </Panel>
        <Panel
          title="Contributors"
          description={git.ownership.note}
          table={
            <DataTable
              rows={contributors}
              rowKey={(c) => c.id}
              initialSort={{ key: "commits", descending: true }}
              columns={[
                { key: "name", header: "Contributor", cell: (c) => c.name, sort: (c) => c.name },
                {
                  key: "commits",
                  header: "Commits",
                  cell: (c) => thousands(c.commits),
                  sort: (c) => c.commits,
                  numeric: true,
                },
                {
                  key: "days",
                  header: "Active days",
                  cell: (c) => thousands(c.activeDays),
                  sort: (c) => c.activeDays,
                  numeric: true,
                },
                {
                  key: "first",
                  header: "First",
                  cell: (c) => date(c.firstCommit),
                  sort: (c) => c.firstCommit,
                },
                {
                  key: "last",
                  header: "Last",
                  cell: (c) => date(c.lastCommit),
                  sort: (c) => c.lastCommit,
                },
                {
                  key: "areas",
                  header: "Main areas",
                  cell: (c) => (
                    <span className="path">
                      {c.areas
                        .slice(0, 3)
                        .map((a) => a.path || "(root)")
                        .join(", ")}
                    </span>
                  ),
                },
              ]}
            />
          }
        >
          <BarList
            label="Commits per contributor"
            unit={[" commit", " commits"]}
            bars={contributors.slice(0, 10).map((c) => ({
              label: c.name,
              value: c.commits,
              details: `${date(c.firstCommit)} to ${date(c.lastCommit)} · ${c.activeDays} active days`,
            }))}
          />
          {contributors.length > 10 ? (
            <p className="muted">Top 10 of {contributors.length}; the table lists everyone.</p>
          ) : null}
        </Panel>
      </div>

      <Panel
        title="Where work happens"
        description="Directories by commits, with the share of their changes made recently."
      >
        <DataTable
          rows={git.directoryActivity}
          rowKey={(d) => d.path}
          initialSort={{ key: "commits", descending: true }}
          limit={15}
          columns={[
            {
              key: "path",
              header: "Directory",
              cell: (d) => <span className="path">{d.path || "(root)"}</span>,
              sort: (d) => d.path,
            },
            {
              key: "commits",
              header: "Commits",
              cell: (d) => thousands(d.commits),
              sort: (d) => d.commits,
              numeric: true,
            },
            {
              key: "churn",
              header: "Lines changed",
              cell: (d) => thousands(d.churn),
              sort: (d) => d.churn,
              numeric: true,
            },
            {
              key: "authors",
              header: "Authors",
              cell: (d) => thousands(d.authors),
              sort: (d) => d.authors,
              numeric: true,
            },
            {
              key: "recent",
              header: `Changed in last ${git.recentWindowDays} days`,
              cell: (d) => percent(d.recentChurnShare),
              sort: (d) => d.recentChurnShare,
              numeric: true,
            },
            {
              key: "last",
              header: "Last change",
              cell: (d) => date(d.lastChanged),
              sort: (d) => d.lastChanged,
            },
          ]}
        />
      </Panel>

      <Panel title="Recent commits">
        <DataTable
          rows={git.commits}
          rowKey={(c) => c.hash}
          limit={15}
          columns={[
            {
              key: "date",
              header: "Date",
              cell: (c) => date(c.timestamp),
              sort: (c) => c.timestamp,
            },
            { key: "hash", header: "Commit", cell: (c) => <span className="mono">{c.short}</span> },
            {
              key: "author",
              header: "Author",
              cell: (c) => author(c.author),
              sort: (c) => author(c.author),
            },
            { key: "subject", header: "Subject", cell: (c) => <Omittable text={c.subject} /> },
            {
              key: "files",
              header: "Files",
              cell: (c) => thousands(c.filesChanged),
              sort: (c) => c.filesChanged,
              numeric: true,
            },
            {
              key: "lines",
              header: "Lines",
              cell: (c) => `+${thousands(c.insertions)} −${thousands(c.deletions)}`,
              sort: (c) => c.insertions + c.deletions,
              numeric: true,
            },
          ]}
        />
      </Panel>

      <div className="grid two" style={{ marginTop: 16 }}>
        <Panel title="Releases">
          {git.releases.length === 0 ? (
            <p className="muted">No version tags were found.</p>
          ) : (
            <DataTable
              rows={git.releases}
              rowKey={(r) => r.tag}
              columns={[
                { key: "tag", header: "Tag", cell: (r) => <span className="mono">{r.tag}</span> },
                { key: "date", header: "Date", cell: (r) => date(r.date), sort: (r) => r.date },
                {
                  key: "since",
                  header: "Commits since previous",
                  cell: (r) => thousands(r.commitsSincePrevious),
                  sort: (r) => r.commitsSincePrevious,
                  numeric: true,
                },
              ]}
            />
          )}
        </Panel>
        <Panel title="Quiet periods" description="The longest gaps between consecutive commits.">
          {git.dormantPeriods.length === 0 ? (
            <p className="muted">No long gaps between commits.</p>
          ) : (
            <DataTable
              rows={git.dormantPeriods}
              rowKey={(p) => `${p.start}-${p.end}`}
              columns={[
                { key: "start", header: "From", cell: (p) => date(p.start), sort: (p) => p.start },
                { key: "end", header: "To", cell: (p) => date(p.end), sort: (p) => p.end },
                {
                  key: "days",
                  header: "Length",
                  cell: (p) => span(p.days),
                  sort: (p) => p.days,
                  numeric: true,
                },
              ]}
            />
          )}
        </Panel>
      </div>

      {git.branches.length > 0 ? (
        <Panel
          title="Branches"
          description={git.defaultBranch ? `Default branch: ${git.defaultBranch}` : undefined}
        >
          <DataTable
            rows={git.branches}
            rowKey={(b) => `${b.remote ? "remote" : "local"}:${b.name}`}
            limit={10}
            columns={[
              {
                key: "name",
                header: "Branch",
                cell: (b) => <span className="mono">{b.name}</span>,
                sort: (b) => b.name,
              },
              { key: "where", header: "Where", cell: (b) => (b.remote ? "remote" : "local") },
              {
                key: "updated",
                header: "Last commit",
                cell: (b) => date(b.updated),
                sort: (b) => b.updated,
              },
            ]}
          />
        </Panel>
      ) : null}
    </>
  );
}
