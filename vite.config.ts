import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],

  // Tauri開発時にRustのエラーを隠さない。
  clearScreen: false,
  // Tauriが参照するポートを固定し、競合時は即座に失敗させる。
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Rust側の変更監視はTauriへ任せる。
      ignored: ["**/src-tauri/**"],
    },
  },
}));
