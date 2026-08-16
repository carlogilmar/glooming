import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],

  // Vite options tailored for Tauri development.
  // 1. don't let Vite obscure Rust errors
  clearScreen: false,
  // 2. Tauri expects a fixed port and fails if it isn't available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    // 3. src-tauri is Rust's; Vite shouldn't watch it
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
