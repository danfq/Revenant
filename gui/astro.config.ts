import { defineConfig } from "astro/config"
import react from "@astrojs/react"
import process from "node:process"

const host = process.env.TAURI_DEV_HOST

export default defineConfig({
  integrations: [react()],
  output: "static",
  server: { port: 1420, host: host || false },
  devToolbar: { enabled: false },
  vite: {
    clearScreen: false,
    server: {
      strictPort: true,
      hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
      watch: { ignored: ["**/src-tauri/**"] },
    },
  },
})
