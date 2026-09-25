// The desktop app's backend: the same operations as the local server, answered by the
// Rust core through Tauri commands instead of HTTP.

import { invoke } from "@tauri-apps/api/core";
import { checkArtifact, type RepositoryDna } from "@repodna/schema";
import type {
  Backend,
  JobState,
  ReportOptions,
  RepositorySummary,
  SaveFormat,
  ScanSummary,
  Session,
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

  startScan(input: string, profile?: string): Promise<JobState> {
    return invoke<JobState>("start_scan", { input, profile: profile ?? null });
  }

  job(id: string): Promise<JobState> {
    return invoke<JobState>("get_job", { id });
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

  async openExternal(url: string): Promise<void> {
    await invoke("open_link", { url });
  }
}
