import { existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, searchForWorkspaceRoot } from 'vite'

// `$pro` = the Chitach Pro UI (src/lib/pro) when it is present, otherwise the empty
// stand-in (src/lib/pro-stub, Chitach Free). CHITACH_PRO=0 forces Free.
const proDir = fileURLToPath(new URL('./src/lib/pro', import.meta.url))
const withPro = process.env.CHITACH_PRO !== '0' && existsSync(`${proDir}/index.ts`)
const proModule = withPro ? `${proDir}/index.ts` : fileURLToPath(new URL('./src/lib/pro-stub/index.ts', import.meta.url))

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  resolve: {
    alias: { $pro: proModule },
  },
  server: {
    fs: {
      // The mock API reads the fines provider definitions from src-tauri/providers.
      allow: [searchForWorkspaceRoot(process.cwd()), '../src-tauri/providers'],
    },
  },
})
