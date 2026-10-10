import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Cau hinh Vite cho Muster.
// Cong 1420 phai khop `build.devUrl` trong crates/muster-app/src-tauri/tauri.conf.json.
export default defineConfig({
  plugins: [react()],

  // Tauri hien thi log cua no, dung de Vite xoa man hinh.
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // src-tauri do cargo theo doi, Vite khong can quet lai.
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // WebView2 la evergreen tren Windows nen nham thang vao Chromium hien dai.
    target: "chrome120",
    sourcemap: false,
    outDir: "dist",
  },
});
