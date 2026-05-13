#!/usr/bin/env node
/**
 * Multi-resolution screenshot capture for LUMEN Visual Critique.
 *
 * Captures the landing at the 5 standard breakpoints (320, 768, 1024,
 * 1440, 1920 px wide) and writes PNGs to test-results/visual/. Use as
 * the artifact input for the Visual Critique Multi-Resolution phase in
 * LUMEN v0.11.0+.
 *
 * Usage:
 *   npm run dev      # in another shell
 *   node scripts/visual-critique.mjs               # screenshots only
 *   node scripts/visual-critique.mjs --url URL     # custom URL
 *
 * Exit codes:
 *   0 — all screenshots captured successfully
 *   1 — at least one screenshot failed
 */

import { chromium } from 'playwright';
import { mkdir, writeFile } from 'node:fs/promises';
import { argv } from 'node:process';
import path from 'node:path';

const DEFAULT_URL = 'http://localhost:4321/seele';

const PAGES = [
  { path: '', name: 'home' },
  { path: '/observability', name: 'observability' },
];

const VIEWPORTS = [
  { width: 320,  height: 720,  label: '320-mobile-narrow' },
  { width: 768,  height: 1024, label: '768-tablet' },
  { width: 1024, height: 768,  label: '1024-tablet-landscape' },
  { width: 1440, height: 900,  label: '1440-laptop' },
  { width: 1920, height: 1080, label: '1920-desktop' },
];

const THEMES = [
  { value: 'dark', label: 'dark', colorScheme: 'dark' },
  { value: 'light', label: 'light', colorScheme: 'light' },
];

function parseArgs() {
  let url = DEFAULT_URL;
  for (let i = 2; i < argv.length; i++) {
    if (argv[i] === '--url' && argv[i + 1]) {
      url = argv[i + 1];
      i++;
    }
  }
  return { url };
}

async function captureOne(browser, url, vp, theme, outDir, fileLabel) {
  const context = await browser.newContext({
    viewport: { width: vp.width, height: vp.height },
    deviceScaleFactor: 1,
    reducedMotion: 'reduce', // honor prefers-reduced-motion (a11y)
    colorScheme: theme.colorScheme,
  });
  const page = await context.newPage();
  // Force theme override via localStorage before page load
  await context.addInitScript((t) => {
    try { localStorage.setItem('seele-theme', t); } catch (_) {}
  }, theme.value);
  try {
    await page.goto(url, { waitUntil: 'networkidle', timeout: 15_000 });
    await page.waitForTimeout(800);
    try {
      await page.waitForFunction(
        () => !document.querySelector('[data-obs]')
          || document.querySelector('[data-obs]')?.getAttribute('data-state') !== 'probing',
        { timeout: 3_000 },
      );
    } catch {
      // probe took too long; capture anyway
    }
    await page.waitForTimeout(200);
    const buf = await page.screenshot({ fullPage: true, type: 'png' });
    const file = path.join(outDir, `${fileLabel}.png`);
    await writeFile(file, buf);
    console.log(`  [OK]  ${fileLabel.padEnd(50)} ${buf.length} bytes`);
    return { ok: true, file, bytes: buf.length };
  } catch (err) {
    console.log(`  [X]   ${fileLabel.padEnd(50)} ERROR: ${err.message}`);
    return { ok: false, error: err.message };
  } finally {
    await context.close();
  }
}

async function main() {
  const { url } = parseArgs();
  const outDir = path.resolve('test-results', 'visual');
  await mkdir(outDir, { recursive: true });

  // Base URL has no trailing slash; each page path starts with /
  const baseUrl = url.replace(/\/+$/, '');

  console.log(`Visual Critique Multi-Resolution`);
  console.log(`Base:    ${baseUrl}`);
  console.log(`Output:  ${outDir}`);
  console.log(`Matrix:  ${PAGES.length} pages × ${VIEWPORTS.length} viewports × ${THEMES.length} themes = ${PAGES.length * VIEWPORTS.length * THEMES.length} shots`);
  console.log();

  const browser = await chromium.launch({ headless: true });
  try {
    const results = [];
    for (const pg of PAGES) {
      for (const theme of THEMES) {
        for (const vp of VIEWPORTS) {
          const pageUrl = `${baseUrl}${pg.path}`;
          const fileLabel = `${pg.name}-${theme.label}-${vp.label}`;
          results.push(await captureOne(browser, pageUrl, vp, theme, outDir, fileLabel));
        }
      }
    }
    const failed = results.filter((r) => !r.ok);
    console.log();
    console.log(`Done: ${results.length - failed.length} ok, ${failed.length} failed`);
    if (failed.length > 0) process.exit(1);
  } finally {
    await browser.close();
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
