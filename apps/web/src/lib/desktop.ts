// The desktop app's backend: the same operations as the local server, answered by the
// Rust core through Tauri commands instead of HTTP.

import { invoke } from "@tauri-apps/api/core";
import { checkArtifact, type RepositoryDna } from "@repodna/schema";
import { checkScanJob, checkScanJobs, checkSourceReferences } from "./backend";
import type {
  Backend,
  JobState,
  ReportOptions,
  RemoteRevision,
  RepositorySummary,
  SaveFormat,
  ScanSummary,
  Session,
  SourceReferences,
  SourceRefQuery,
} from "./backend";

export class DesktopBackend implements Backend {
  readonly kind = "desktop" as const;

  session(): Promise<Session> {
    return invoke<Session>("session");
  }

  repositories(): Promise<RepositorySummary[]> {
    return invoke<RepositorySummary[]>("list_repositories");
  }

  repository(id: string): Promise<{ repository: RepositorySummary; scans: ScanSummary[] }> {
    return invoke("get_repository", { id });
  }

  async artifact(id: string, scan?: string): Promise<RepositoryDna> {
    const value = await invoke<unknown>("load_artifact", { id, scan: scan ?? null });
    return checkArtifact(value).artifact;
  }

  async deleteRepository(id: string): Promise<void> {
    await invoke("delete_repository", { id });
  }

  async deleteScan(id: string, scan: string): Promise<void> {
    await invoke("delete_scan", { id, scan });
  }

  async listRemoteReferences(input: string, query?: SourceRefQuery): Promise<SourceReferences> {
    return checkSourceReferences(
      await invoke<unknown>("list_remote_references", {
        input,
        kind: query?.kind ?? null,
        search: query?.search ?? null,
      }),
    );
  }

  startScan(input: string, profile?: string, revision?: RemoteRevision): Promise<JobState> {
    return invoke<JobState>("start_scan", {
      input,
      profile: profile ?? null,
      branch: revision?.kind === "branch" ? revision.value : null,
      tag: revision?.kind === "tag" ? revision.value : null,
      commit: revision?.kind === "commit" ? revision.value : null,
    });
  }

  async job(id: string): Promise<JobState> {
    return checkScanJob(await invoke<unknown>("get_job", { id }), id);
  }

  async listJobs(): Promise<JobState[]> {
    return checkScanJobs(await invoke<unknown>("list_jobs"));
  }

  async cancelJob(id: string): Promise<void> {
    await invoke("cancel_job", { id });
  }

  reportUrl(): null {
    return null;
  }

  card(id: string, dark: boolean, scan?: string): Promise<string> {
    return invoke<string>("render_card", { id, dark, scan: scan ?? null });
  }

  pickDirectory(): Promise<string | null> {
    return invoke<string | null>("pick_directory");
  }

  saveReport(id: string, format: SaveFormat, options: ReportOptions = {}): Promise<string | null> {
    return invoke<string | null>("save_report", {
      id,
      format,
      scan: options.scan ?? null,
      theme: options.theme ?? null,
      privacy: options.privacy ?? null,
    });
  }

  saveCard(id: string, dark: boolean, png: boolean, scan?: string): Promise<string | null> {
    return invoke<string | null>("save_card", { id, dark, png, scan: scan ?? null });
  }

  async openExternal(url: string): Promise<void> {
    await invoke("open_link", { url });
  }
}

/** Saves a file the page made through a save dialog: where it was saved, or null if not. */
export function saveFile(name: string, text: string): Promise<string | null> {
  return invoke<string | null>("save_file", { name, text });
}
