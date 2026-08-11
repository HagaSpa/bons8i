import { defineConfig } from "vitest/config";
import type { ViteDevServer } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";

const myPlugin = () => ({
  name: "configure-mock-server",
  configureServer(server: ViteDevServer) {
    server.middlewares.use((req, res, next) => {
      if (req.url === "/api/uptime") {
        const data = fs.readFileSync("dev/api/uptime.json", "utf-8");
        res.statusCode = 200;
        res.setHeader("Content-Type", "application/json");
        res.end(data);
        return;
      }
      if (req.url === "/api/status") {
        const data = fs.readFileSync("dev/api/status.json", "utf-8");
        res.statusCode = 200;
        res.setHeader("Content-Type", "application/json");
        res.end(data);
        return;
      }
      next();
    });
  },
});

// /api は起動済みの BFF（`LISTEN_ADDR=127.0.0.1:18080 cargo run`）へ流す
export default defineConfig({
  plugins: [react(), myPlugin()],
  server: {
    proxy: {
      "/api": {
        target: process.env.VITE_API_TARGET ?? "http://127.0.0.1:18080",
      },
    },
  },
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: "logic",
          environment: "node",
          include: ["src/**/*.test.ts"],
        },
      },
      {
        extends: true,
        test: {
          name: "ui",
          environment: "happy-dom",
          include: ["src/**/*.test.tsx"],
          setupFiles: ["./src/test/setup.ts"],
        },
      },
    ],
  },
});
