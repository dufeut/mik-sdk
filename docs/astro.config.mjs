// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import rehypeMermaid from "rehype-mermaid";

// https://astro.build/config
export default defineConfig({
  markdown: {
    rehypePlugins: [
      [
        rehypeMermaid,
        {
          strategy: "img-svg", // Embed as <img> to avoid style conflicts
          dark: true, // Generate light/dark variants with <picture>
        },
      ],
    ],
    // Exclude mermaid from syntax highlighting (Astro 5.5+)
    syntaxHighlight: {
      excludeLangs: ["mermaid"],
    },
  },
  integrations: [
    starlight({
      title: "mik-sdk",
      description:
        "Portable WASI HTTP SDK using Component Composition",
      social: [
        {
          icon: "github",
          label: "GitHub",
          href: "https://github.com/hlop3z/mik-sdk",
        },
      ],
      logo: {
        src: "./src/assets/logo.png",
        replacesTitle: false,
      },
      customCss: ["./src/styles/custom.css"],
      sidebar: [
        { label: "Introduction", slug: "index" },
        {
          label: "Guides",
          items: [
            { label: "Installation", slug: "guides/installation" },
            { label: "Quickstart", slug: "guides/quickstart" },
            { label: "Routing", slug: "guides/routing" },
            { label: "Testing", slug: "guides/testing" },
            { label: "Sidecars Services", slug: "guides/sidecars" },
            { label: "Troubleshooting", slug: "guides/troubleshooting" },
          ],
        },
        {
          label: "Reference",
          items: [
            { label: "Quick Reference", slug: "reference/quick-reference" },
            { label: "Request", slug: "reference/request" },
            { label: "Responses", slug: "reference/responses" },
            { label: "Error Types", slug: "reference/errors" },
            { label: "HTTP Client", slug: "reference/http-client" },
            { label: "SQL Macros", slug: "reference/sql" },
            { label: "Date & Time", slug: "reference/datetime" },
            { label: "Random", slug: "reference/random" },
            { label: "Environment", slug: "reference/environment" },
            { label: "Logging", slug: "reference/logging" },
          ],
        },
        {
          label: "Examples",
          items: [
            { label: "Basic", slug: "examples/basic" },
            { label: "CRUD API", slug: "examples/crud" },
          ],
        },
        {
          label: "Best Practices",
          items: [
            { label: "Service Design", slug: "practices/service-design" },
            { label: "Custom Helpers", slug: "practices/helpers" },
            { label: "Common Patterns", slug: "practices/patterns" },
          ],
        },
        { label: "Architecture", slug: "architecture" },
      ],
      pagefind: true,
      editLink: {
        baseUrl: "https://github.com/hlop3z/mik-sdk/edit/main/docs/",
      },
      lastUpdated: true,
    }),
  ],
});
