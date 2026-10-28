/**
 * shared types and log parsing for the recovery ui
 *
 * the backend streams the same plain text lines the cli prints; the ui reads
 * them for a live log and derives headline counts from the stable prefixes
 */

import { invoke } from "@tauri-apps/api/core"

/** the four recovery steps, in the order the pipeline runs them */
export type StepKey = "decrypt" | "carve" | "thumbnails" | "media"

export const STEPS: StepKey[] = ["decrypt", "carve", "thumbnails", "media"]

/** per step progress, driven from the streamed log and the final result */
export type Status = "idle" | "running" | "done" | "warn" | "error"

/** short human copy for each step */
export const STEP_INFO: Record<StepKey, { title: string; blurb: string }> = {
  decrypt: { title: "Decrypt", blurb: "unlock Manifest.db and Photos.sqlite" },
  carve: { title: "Carve", blurb: "recover deleted records from free pages" },
  thumbnails: { title: "Thumbnails", blurb: "hidden, recently deleted and purged" },
  media: { title: "Media", blurb: "decrypt originals that are present" },
}

export type LogLevel = "ok" | "done" | "info" | "note" | "count" | "warn" | "stop" | "busy" | "plain"

export type LogLine = { level: LogLevel; text: string }

/** tag a line by its leading marker so the console can colour it */
export function classify(text: string): LogLevel {
  const s = text.trimStart()
  if (s.startsWith("[ok]")) return "ok"
  if (s.startsWith("[done]")) return "done"
  if (s.startsWith("[info]")) return "info"
  if (s.startsWith("[note]")) return "note"
  if (s.startsWith("[count")) return "count"
  if (s.startsWith("[warn]")) return "warn"
  if (s.startsWith("[stop]")) return "stop"
  if (s.startsWith("[..]")) return "busy"
  return "plain"
}

export type MediaGroup = { label: string; ok: number; total: number }

/** headline numbers pulled out of the run, filled in as lines arrive */
export type Results = {
  classKeys: number | null
  deletedRecords: number | null
  orphan: number | null
  hidden: number | null
  recentlyDeleted: number | null
  media: MediaGroup[]
}

export function emptyResults(): Results {
  return { classKeys: null, deletedRecords: null, orphan: null, hidden: null, recentlyDeleted: null, media: [] }
}

const RE = {
  classKeys: /Manifest\.db decrypted\s+\((\d+) class keys/,
  deleted: /deleted ZASSET rows recovered from free pages:\s*(\d+)/,
  thumbs: /orphan=(\d+)\s+hidden-album=(\d+)\s+recently-deleted=(\d+)/,
  mediaOk: /\[ok\]\s+(\S+)\s+(\d+)\/(\d+)\s+files/,
  mediaCount: /\[count\]\s+(\S+)\s+(\d+)\s+files present/,
}

/** merge anything parseable from one line into the running results */
export function ingest(r: Results, text: string): Results {
  const next = { ...r, media: [...r.media] }
  let m: RegExpMatchArray | null

  if ((m = text.match(RE.classKeys))) next.classKeys = Number(m[1])
  if ((m = text.match(RE.deleted))) next.deletedRecords = Number(m[1])
  if ((m = text.match(RE.thumbs))) {
    next.orphan = Number(m[1])
    next.hidden = Number(m[2])
    next.recentlyDeleted = Number(m[3])
  }
  if ((m = text.match(RE.mediaOk))) mergeMedia(next.media, m[1], Number(m[2]), Number(m[3]))
  else if ((m = text.match(RE.mediaCount))) mergeMedia(next.media, m[1], Number(m[2]), Number(m[2]))

  return next
}

function mergeMedia(groups: MediaGroup[], label: string, ok: number, total: number) {
  const existing = groups.find((g) => g.label === label)
  if (existing) {
    existing.ok = ok
    existing.total = total
  } else {
    groups.push({ label, ok, total })
  }
}

/** friendly names for the raw media group labels */
export const MEDIA_LABELS: Record<string, string> = {
  originals_DCIM: "Originals",
  cached_renders_UBF: "Cached renders",
  edit_mutations: "Edit mutations",
  cpl_assets: "iCloud-staged",
}

export type RunOptions = { countOnly: boolean; force: boolean; includeAll: boolean }

export type RunParams = {
  backup: string
  workspace: string
  password: string
  step: StepKey | "all"
  countOnly: boolean
  force: boolean
  includeAll: boolean
}

/** run a step (or the whole pipeline); resolves on success, rejects with a message */
export function runRecovery(params: RunParams): Promise<void> {
  return invoke("run_recovery", { params })
}

export function defaultWorkspace(): Promise<string> {
  return invoke("default_workspace")
}

export function reveal(path: string): Promise<void> {
  return invoke("reveal", { path })
}
