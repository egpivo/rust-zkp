import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://egpivo.github.io',
  base: '/rust-zkp',
  markdown: {
    syntaxHighlight: 'shiki',
    shikiConfig: {
      theme: 'github-light',
    },
  },
});
