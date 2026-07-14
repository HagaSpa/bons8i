import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// /api は起動済みの BFF（`LISTEN_ADDR=127.0.0.1:18080 cargo run`）へ流す
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
