/**
 * inline svg icons
 *
 * all stroke with currentColor so they inherit text colour and theme
 */

import type { StepKey } from "./lib/recovery"

const stroke = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.6,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
}

/** a photo returning from a faded echo: the revenant idea */
export function BrandMark() {
  return (
    <svg className="mark" viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
      <rect x="6" y="9" width="22" height="18" rx="4" className="mark-ghost" />
      <rect x="12" y="13" width="22" height="18" rx="4" className="mark-frame" />
      <circle cx="19" cy="20" r="2.4" className="mark-accent" />
      <path d="M15 29l6-6 4 3 4-4 5 5" className="mark-accent" fill="none" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

export function FolderIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true" {...stroke}>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  )
}

export function EyeIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true" {...stroke}>
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7z" />
      <circle cx="12" cy="12" r="3" />
    </svg>
  )
}

export function EyeOffIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true" {...stroke}>
      <path d="M3 3l18 18" />
      <path d="M10.6 6.2A9.8 9.8 0 0 1 12 5c6.5 0 10 7 10 7a16 16 0 0 1-3.2 3.9" />
      <path d="M6.3 7.4A16 16 0 0 0 2 12s3.5 7 10 7a9.6 9.6 0 0 0 4.3-1" />
      <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2" />
    </svg>
  )
}

export function TickIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true" {...stroke}>
      <path d="M5 12.5l4.5 4.5L19 7" strokeWidth="2" />
    </svg>
  )
}

/** open the output folder in the system file manager */
export function OpenIcon() {
  return (
    <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true" {...stroke}>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h5a2 2 0 0 1 2 2v2" />
      <path d="M3 10h15.5a1.5 1.5 0 0 1 1.45 1.9l-1.4 5A2 2 0 0 1 16.6 18H5a2 2 0 0 1-2-2z" />
    </svg>
  )
}

export function StepIcon({ step }: { step: StepKey }) {
  switch (step) {
    case "decrypt":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" {...stroke}>
          <rect x="5" y="11" width="14" height="9" rx="2" />
          <path d="M8 11V8a4 4 0 0 1 7.3-2.3" />
          <circle cx="12" cy="15.5" r="1.3" />
        </svg>
      )
    case "carve":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" {...stroke}>
          <ellipse cx="12" cy="6" rx="7" ry="3" />
          <path d="M5 6v6c0 1.7 3.1 3 7 3s7-1.3 7-3V6" />
          <path d="M5 12v6c0 1.7 3.1 3 7 3s7-1.3 7-3v-6" />
        </svg>
      )
    case "thumbnails":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" {...stroke}>
          <rect x="4" y="5" width="16" height="14" rx="2" />
          <circle cx="9" cy="10" r="1.6" />
          <path d="M5 17l4.5-4.5 3 3L16 11l3 3" />
        </svg>
      )
    case "media":
      return (
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" {...stroke}>
          <rect x="7" y="7" width="13" height="13" rx="2" />
          <path d="M4 16V5a1 1 0 0 1 1-1h11" />
          <path d="M10 15l2.5-2.5L15 15l2-2 3 3" />
        </svg>
      )
  }
}
