import { svelte } from '@sveltejs/vite-plugin-svelte'
import { defineConfig, searchForWorkspaceRoot } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte()],
  server: {
    fs: {
      // The mock API reads the fines provider definitions from src-tauri/providers.
      allow: [searchForWorkspaceRoot(process.cwd()), '../src-tauri/providers'],
    },
  },
})
