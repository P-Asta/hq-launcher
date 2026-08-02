import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const projectRoot = path.dirname(fileURLToPath(import.meta.url));

function inlineEmbeddedOverlay() {
  return {
    name: "hq-inline-embedded-overlay",
    apply: "build",
    enforce: "post",
    generateBundle(_options, bundle) {
      const htmlEntry = Object.values(bundle).find(
        (item) => item.type === "asset" && item.fileName.endsWith("embedded-overlay.html"),
      );
      if (!htmlEntry) throw new Error("Embedded overlay HTML output was not generated.");

      let html = String(htmlEntry.source);
      for (const [fileName, item] of Object.entries(bundle)) {
        if (item === htmlEntry) continue;
        const escapedName = fileName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        if (item.type === "chunk") {
          if (!item.isEntry) {
            throw new Error(`Embedded overlay build produced an unexpected chunk: ${fileName}`);
          }
          const scriptPattern = new RegExp(
            `<script([^>]*?)src=["'](?:\\./|/)?${escapedName}["']([^>]*)><\\/script>`,
          );
          const code = item.code.replace(/<\/script/gi, "<\\/script");
          let inlined = false;
          html = html.replace(scriptPattern, (_match, beforeSrc, afterSrc) => {
            inlined = true;
            return `<script${beforeSrc}${afterSrc}>\n${code}\n</script>`;
          });
          if (!inlined) throw new Error(`Could not inline embedded overlay script: ${fileName}`);
          delete bundle[fileName];
          continue;
        }
        if (item.type === "asset" && fileName.endsWith(".css")) {
          const stylePattern = new RegExp(
            `<link([^>]*?)href=["'](?:\\./|/)?${escapedName}["']([^>]*)>`,
          );
          const css = String(item.source).replace(/<\/style/gi, "<\\/style");
          let inlined = false;
          html = html.replace(stylePattern, () => {
            inlined = true;
            return `<style>\n${css}\n</style>`;
          });
          if (!inlined) throw new Error(`Could not inline embedded overlay stylesheet: ${fileName}`);
          delete bundle[fileName];
        }
      }

      const unexpected = Object.values(bundle).filter((item) => item !== htmlEntry);
      if (unexpected.length > 0) {
        throw new Error(`Embedded overlay build left non-inlined assets: ${unexpected.map((item) => item.fileName).join(", ")}`);
      }
      htmlEntry.source = html;
    },
  };
}

export default defineConfig({
  root: projectRoot,
  base: "./",
  publicDir: false,
  plugins: [react(), inlineEmbeddedOverlay()],
  define: {
    __HQ_LAUNCHER_DEV_ENV__: JSON.stringify(""),
  },
  build: {
    outDir: "dist-embedded-overlay",
    emptyOutDir: true,
    cssCodeSplit: false,
    assetsInlineLimit: 10_000_000,
    sourcemap: false,
    rollupOptions: {
      input: path.resolve(projectRoot, "embedded-overlay.html"),
      output: {
        inlineDynamicImports: true,
        entryFileNames: "assets/hq-overlay.js",
        assetFileNames: "assets/hq-overlay[extname]",
      },
    },
  },
});
