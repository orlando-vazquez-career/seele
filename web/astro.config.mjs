// @ts-check
import { defineConfig } from 'astro/config';

// SEELE landing — Astro 5.x static.
//
// Deployed to GitHub Pages at:
//   https://orlando-vazquez-career.github.io/seele/
//
// `base: '/seele'` makes the dev preview and built paths line up with the
// production subdirectory served by GH Pages from the `gh-pages` branch.
export default defineConfig({
  site: 'https://orlando-vazquez-career.github.io',
  base: '/seele',
  trailingSlash: 'never',
  output: 'static',
  build: {
    inlineStylesheets: 'always',
    assets: 'assets',
  },
  vite: {
    build: {
      cssCodeSplit: false,
    },
  },
});
