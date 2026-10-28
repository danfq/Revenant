import { Fragment, useEffect, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { open as openDialog } from "@tauri-apps/plugin-dialog"
import {
  classify,
  defaultWorkspace,
  emptyResults,
  ingest,
  MEDIA_LABELS,
  reveal,
  runRecovery,
  STEP_INFO,
  STEPS,
  type LogLine,
  type Results,
  type Status,
  type StepKey,
} from "./lib/recovery"
import { lockDownDesktop } from "./lib/desktop"
import { BrandMark, EyeIcon, EyeOffIcon, FolderIcon, OpenIcon, StepIcon, TickIcon } from "./icons"
import "./App.css"

/** pick the closest desktop platform so the ui can match it */
function detectOs(): string {
  const ua = navigator.userAgent
  if (/Mac/i.test(ua)) return "macos"
  if (/Win/i.test(ua)) return "windows"
  if (/Linux|X11|CrOS/i.test(ua)) return "linux"
  return "other"
}

function App() {
  const [backup, setBackup] = useState<string | null>(null)
  const [workspace, setWorkspace] = useState("")
  const [password, setPassword] = useState("")
  const [showPassword, setShowPassword] = useState(false)
  const [showOptions, setShowOptions] = useState(false)
  const [countOnly, setCountOnly] = useState(false)
  const [force, setForce] = useState(false)
  const [includeAll, setIncludeAll] = useState(false)

  const [log, setLog] = useState<LogLine[]>([])
  const [results, setResults] = useState<Results>(emptyResults())
  const [status, setStatus] = useState<Record<StepKey, Status>>({
    decrypt: "idle",
    carve: "idle",
    thumbnails: "idle",
    media: "idle",
  })
  const [running, setRunning] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [hasRun, setHasRun] = useState(false)

  // the listener is bound once, so step progression reads from a ref
  const cursor = useRef<{ order: StepKey[]; idx: number } | null>(null)
  const logEnd = useRef<HTMLDivElement>(null)

  useEffect(() => {
    document.documentElement.dataset.os = detectOs()
    const release = lockDownDesktop()

    defaultWorkspace()
      .then(setWorkspace)
      .catch(() => {})

    const unlisten = listen<string>("rv:log", (event) => {
      const text = event.payload
      setLog((prev) => [...prev, { level: classify(text), text }])
      setResults((prev) => ingest(prev, text))
      advance(text)
    })
    return () => {
      release()
      unlisten.then((off) => off())
    }
  }, [])

  useEffect(() => {
    logEnd.current?.scrollIntoView({ block: "end" })
  }, [log])

  // move the pipeline cursor when a step finishes mid run
  function advance(text: string) {
    const c = cursor.current
    if (!c) return
    const done = text.startsWith("[done]")
    const stopped = text.startsWith("[stop]") || text.startsWith("[warn]")
    if (!done && !stopped) return

    const step = c.order[c.idx]
    if (!step) return
    setStatus((s) => ({ ...s, [step]: done ? "done" : "warn" }))
    c.idx += 1
    const next = c.order[c.idx]
    if (next) setStatus((s) => ({ ...s, [next]: "running" }))
  }

  const canRun = Boolean(backup) && password.length > 0 && workspace.length > 0 && !running

  async function pick(kind: "backup" | "workspace") {
    const picked = await openDialog({
      directory: true,
      multiple: false,
      title: kind === "backup" ? "Choose the encrypted backup folder" : "Choose where to save recovered files",
    })
    if (typeof picked !== "string") return
    if (kind === "backup") setBackup(picked)
    else setWorkspace(picked)
  }

  async function run() {
    if (!backup || !canRun) return

    cursor.current = { order: STEPS, idx: 0 }
    setStatus({ decrypt: "running", carve: "idle", thumbnails: "idle", media: "idle" })
    setLog([{ level: "busy", text: "[..] starting recovery" }])
    setResults(emptyResults())
    setError(null)
    setRunning(true)
    setHasRun(true)

    try {
      await runRecovery({ backup, workspace, password, step: "all", countOnly, force, includeAll })
      setStatus((s) => mapStatus(s, "running", "done"))
    } catch (e) {
      const raw = String(e)
      setStatus((s) => mapStatus(s, "running", "error"))
      setError(humanize(raw))
      setLog((prev) => [...prev, { level: "stop", text: `[error] ${raw}` }])
    } finally {
      cursor.current = null
      setRunning(false)
    }
  }

  return (
    <div className="shell">
      <header className="topbar">
        <div className="brand">
          <BrandMark />
          <div className="brand-text">
            <h1>Revenant</h1>
            <p>Recover deleted iOS photos, privately and offline</p>
          </div>
        </div>
        <span className="chip" title="Nothing leaves your device">
          <span className="chip-dot" />
          Local only
        </span>
      </header>

      <main className="main">
        <aside className="sidebar">
          <div className="field">
            <label>Encrypted backup</label>
            <button type="button" className={`picker ${backup ? "set" : ""}`} onClick={() => pick("backup")}>
              <FolderIcon />
              <span className="picker-text">{backup ? basename(backup) : "Choose backup folder"}</span>
            </button>
            <p className="hint">{backup ?? "The folder that holds Manifest.plist"}</p>
          </div>

          <div className="field">
            <label htmlFor="pw">Backup password</label>
            <div className="password">
              <input
                id="pw"
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(e) => setPassword(e.currentTarget.value)}
                placeholder="Encrypt local backup password"
                autoComplete="off"
                spellCheck={false}
              />
              <button type="button" className="reveal" onClick={() => setShowPassword((v) => !v)} aria-label="Show password">
                {showPassword ? <EyeOffIcon /> : <EyeIcon />}
              </button>
            </div>
            <p className="hint">The password set when you turned on Encrypt local backup</p>
          </div>

          <div className="field">
            <label>Save recovered files to</label>
            <button type="button" className={`picker ${workspace ? "set" : ""}`} onClick={() => pick("workspace")}>
              <FolderIcon />
              <span className="picker-text">{workspace ? basename(workspace) : "Choose a folder"}</span>
            </button>
            <p className="hint">{workspace || "A folder for decrypted databases and output"}</p>
          </div>

          <button type="button" className="disclosure" onClick={() => setShowOptions((v) => !v)}>
            <span className={`caret ${showOptions ? "open" : ""}`} />
            Options
          </button>
          {showOptions && (
            <div className="options">
              <Check label="Count only, write no files" checked={countOnly} onChange={setCountOnly} />
              <Check label="Force thumbnails on low match rate" checked={force} onChange={setForce} />
              <Check label="Include iCloud-staged assets" checked={includeAll} onChange={setIncludeAll} />
            </div>
          )}

          <div className="actions">
            <button type="button" className="run-all" disabled={!canRun} onClick={run}>
              {running ? "Recovering…" : "Recover everything"}
            </button>
            {!canRun && !running && <p className="hint center">Choose a backup and enter its password to begin</p>}
          </div>
        </aside>

        <section className="content">
          <ol className="stepper">
            {STEPS.map((key, i) => (
              <Fragment key={key}>
                {i > 0 && <span className={`rail ${status[STEPS[i - 1]] === "done" ? "lit" : ""}`} />}
                <li className={`step ${status[key]}`} title={STEP_INFO[key].blurb}>
                  <span className="dot">{status[key] === "done" ? <TickIcon /> : <StepIcon step={key} />}</span>
                  <span className="step-label">{STEP_INFO[key].title}</span>
                </li>
              </Fragment>
            ))}
          </ol>

          {error && (
            <div className="banner error" role="alert">
              <strong>Recovery stopped.</strong> {error}
            </div>
          )}

          {hasRun && !error && (
            <div className="summary">
              <Figure label="Deleted" value={results.deletedRecords} strong />
              <Figure label="Orphan" value={results.orphan} strong />
              <Figure label="Hidden" value={results.hidden} />
              <Figure label="Recently deleted" value={results.recentlyDeleted} />
              {results.media.length > 0 && (
                <div className="media">
                  {results.media.map((g) => (
                    <span key={g.label} className="media-item">
                      {MEDIA_LABELS[g.label] ?? g.label} {g.ok}
                      {g.ok !== g.total ? `/${g.total}` : ""}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}

          <div className="console">
            <div className="console-bar">
              <span className="console-title">Activity</span>
              {running && <span className="console-live">running</span>}
              <button type="button" className="console-open" disabled={!hasRun} onClick={() => reveal(workspace).catch(() => {})}>
                <OpenIcon />
                Open folder
              </button>
            </div>
            <div className="console-body">
              {log.length === 0 ? (
                <p className="console-empty">Waiting to start. Choose a backup, then run a recovery.</p>
              ) : (
                log.map((line, i) => (
                  <div key={i} className={`line ${line.level}`}>
                    {line.text}
                  </div>
                ))
              )}
              <div ref={logEnd} />
            </div>
          </div>
        </section>
      </main>
    </div>
  )
}

function Figure({ label, value, strong }: { label: string; value: number | null; strong?: boolean }) {
  return (
    <div className={`figure ${strong ? "strong" : ""}`}>
      <span className="fig-value">{value === null ? "—" : value.toLocaleString()}</span>
      <span className="fig-label">{label}</span>
    </div>
  )
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="check">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.currentTarget.checked)} />
      <span className="box">{checked && <TickIcon />}</span>
      {label}
    </label>
  )
}

/** turn a raw backend error into something a person can act on */
function humanize(message: string): string {
  const m = message.toLowerCase()
  if (m.includes("wrong password") || m.includes("did not decrypt")) {
    return "The backup password looks incorrect, or this backup is not encrypted. Check the password and try again."
  }
  if (m.includes("backupkeybag") || m.includes("manifest.plist")) {
    return "This folder is not an encrypted iOS backup. Choose the folder that contains Manifest.plist."
  }
  if (m.includes("unable to open database") || m.includes("code 14")) {
    return "Could not open the photo database from this backup. It may be incomplete or still syncing. Try making a fresh encrypted backup."
  }
  if (m.includes("sqlite3")) {
    return "The sqlite3 tool is needed to carve deleted records but was not found on this system."
  }
  if (m.includes("run 'decrypt' first")) {
    return "The backup still needs decrypting. Run the recovery again from the start."
  }
  return message.replace(/^error:\s*/i, "")
}

function basename(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] ?? path
}

function mapStatus(s: Record<StepKey, Status>, from: Status, to: Status): Record<StepKey, Status> {
  const out = { ...s }
  for (const key of STEPS) if (out[key] === from) out[key] = to
  return out
}

export default App
