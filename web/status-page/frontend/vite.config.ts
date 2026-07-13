import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// ローカル開発: Rust BFF を `LISTEN_ADDR=127.0.0.1:18080 cargo run` で起動しておく
// （ポートを変えた場合は VITE_API_TARGET で上書き）
export default defineConfig({
  plugins: [react()],
  server: {
    proxy: {
      "/api": {
        target: process.env.VITE_API_TARGET ?? "http://127.0.0.1:18080",
      },
    },
  },
});
