/**
 * keep the webview behaving like a native app, not a browser
 *
 * blocks the context menu, reload, dev tools, find, print, view source, zoom
 * and file drops; native window shortcuts (quit, close, minimise) are left alone
 */

export function lockDownDesktop(): () => void {
  const onContextMenu = (e: Event) => e.preventDefault()

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "F5" || e.key === "F12") {
      e.preventDefault()
      return
    }

    const mod = e.metaKey || e.ctrlKey
    if (!mod) return

    const key = e.key.toLowerCase()
    // dev tools
    if (e.shiftKey && (key === "i" || key === "j" || key === "c")) {
      e.preventDefault()
      return
    }
    // reload, find, find next, print, save page, view source
    if (["r", "f", "g", "p", "s", "u"].includes(key)) {
      e.preventDefault()
      return
    }
    // zoom in, out and reset: the layout is fixed to the window
    if (["=", "-", "+", "0"].includes(key)) {
      e.preventDefault()
    }
  }

  const onDrag = (e: Event) => e.preventDefault()

  window.addEventListener("contextmenu", onContextMenu)
  window.addEventListener("keydown", onKeyDown)
  window.addEventListener("dragover", onDrag)
  window.addEventListener("drop", onDrag)

  return () => {
    window.removeEventListener("contextmenu", onContextMenu)
    window.removeEventListener("keydown", onKeyDown)
    window.removeEventListener("dragover", onDrag)
    window.removeEventListener("drop", onDrag)
  }
}
