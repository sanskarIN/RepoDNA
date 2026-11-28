// The desktop app's backend: the same operations as the local server, answered by the
// Rust core through Tauri commands instead of HTTP.

import { invoke } from "@tauri-apps/api/core";
import type { RepositoryDna } from "@repodna/schema";
import {
  checkHistoryPage,
  checkNativeArtifact,
  checkNewScanJob,
  checkOptionalPath,
  checkRepositories,
  checkRepository,
  checkScanJob,
  checkScanJobs,
  checkSession,
  checkSourceReferences,
  checkSvgCard,
} from "./backend";
import type {
  Backend,
  HistoryPage,
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

  constructor(private readonly call: typeof invoke = invoke) {}

  async session(): Promise<Session> {
    return checkSession(await this.call<unknown>("session"));
  }

  async repositories(): Promise<RepositorySummary[]> {
    return checkRepositories(await this.call<unknown>("list_repositories"));
  }

  async repository(id: string): Promise<{ repository: RepositorySummary; scans: ScanSummary[] }> {
    return checkRepository(await this.call<unknown>("get_repository", { id }), id);
  }

  async history(id: string, after?: string): Promise<HistoryPage> {
    const value = await this.call<unknown>("repository_history", { id, limit: 50, after: after ?? null });
    return checkHistoryPage(value, id, after);
  }

  async artifact(id: string, scan?: string): Promise<RepositoryDna> {
    const value = await this.call<unknown>("load_artifact", { id, scan: scan ?? null });
    return checkNativeArtifact(value, scan);
  }

  async deleteRepository(id: string): Promise<void> {
    await this.call("delete_repository", { id });
  }

  async deleteScan(id: string, scan: string): Promise<void> {
    await this.call("delete_scan", { id, scan });
  }

  async listRemoteReferences(input: string, query?: SourceRefQuery): Promise<SourceReferences> {
    return checkSourceReferences(
      await this.call<unknown>("list_remote_references", {
        input,
        kind: query?.kind ?? null,
        search: query?.search ?? null,
      }),
    );
  }

  async startScan(input: string, profile?: string, revision?: RemoteRevision): Promise<JobState> {
    return checkNewScanJob(await this.call<unknown>("start_scan", {
      input,
      profile: profile ?? null,
      branch: revision?.kind === "branch" ? revision.value : null,
      tag: revision?.kind === "tag" ? revision.value : null,
      commit: revision?.kind === "commit" ? revision.value : null,
    }));
  }

  async job(id: string): Promise<JobState> {
    return checkScanJob(await this.call<unknown>("get_job", { id }), id);
  }

  async listJobs(): Promise<JobState[]> {
    return checkScanJobs(await this.call<unknown>("list_jobs"));
  }

  async cancelJob(id: string): Promise<void> {
    await this.call("cancel_job", { id });
  }

  reportUrl(): null {
    return null;
  }

  async card(id: string, dark: boolean, scan?: string): Promise<string> {
    return checkSvgCard(await this.call<unknown>("render_card", { id, dark, scan: scan ?? null }));
  }

  async pickDirectory(): Promise<string | null> {
    return checkOptionalPath(await this.call<unknown>("pick_directory"));
  }

  async saveReport(id: string, format: SaveFormat, options: ReportOptions = {}): Promise<string | null> {
    return checkOptionalPath(await this.call<unknown>("save_report", {
      id,
      format,
      scan: options.scan ?? null,
      theme: options.theme ?? null,
      privacy: options.privacy ?? null,
    }));
  }

  async saveCard(id: string, dark: boolean, png: boolean, scan?: string): Promise<string | null> {
    return checkOptionalPath(await this.call<unknown>("save_card", { id, dark, png, scan: scan ?? null }));
  }

  async openExternal(url: string): Promise<void> {
    await this.call("open_link", { url });
  }
}

/** Saves a file the page made through a save dialog: where it was saved, or null if not. */
export async function saveFile(name: string, text: string): Promise<string | null> {
  return checkOptionalPath(await invoke<unknown>("save_file", { name, text }));
}
