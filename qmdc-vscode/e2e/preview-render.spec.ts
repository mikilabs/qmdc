import { test, expect } from '@playwright/test';
import { generatePreviewHtml, transformQmdcSyntax, clearSearchIndexCache, getMermaidEnhanceScript, rewriteImageSources } from '../src/preview-renderer';
import * as fs from 'fs';
import * as path from 'path';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Clear search index cache between tests to avoid cross-test contamination
test.beforeEach(() => { clearSearchIndexCache(); });

const FIXTURES = path.join(__dirname, 'fixtures');

function fixture(name: string): string {
  return fs.readFileSync(path.join(FIXTURES, name), 'utf-8');
}

async function renderQmdc(page: import('@playwright/test').Page, qmd: string) {
  const html = await generatePreviewHtml(qmd, null, 'file:///test.qmd.md', {
    includeVscodeApi: false,
  });
  await page.setContent(html, { waitUntil: 'networkidle' });
}

async function renderFixture(page: import('@playwright/test').Page, name: string) {
  await renderQmdc(page, fixture(name));
}

// ---------------------------------------------------------------------------
// Data-driven: basic markdown rendering
// ---------------------------------------------------------------------------

const basicRenderCases: {
  name: string;
  fixture: string;
  selector: string;
  assertion: 'count' | 'text' | 'visible+text';
  expected: string | number;
}[] = [
  { name: 'h1 heading', fixture: 'basic-heading.qmd.md', selector: 'h1', assertion: 'text', expected: 'Hello World' },
  { name: 'paragraphs', fixture: 'basic-paragraphs.qmd.md', selector: 'p', assertion: 'count', expected: 2 },
  { name: 'bullet list', fixture: 'basic-list.qmd.md', selector: 'li', assertion: 'count', expected: 3 },
  { name: 'code block', fixture: 'basic-code-block.qmd.md', selector: 'pre', assertion: 'visible+text', expected: 'console.log' },
];

for (const tc of basicRenderCases) {
  test(`renders ${tc.name}`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const loc = page.locator(tc.selector);
    if (tc.assertion === 'count') {
      await expect(loc).toHaveCount(tc.expected as number);
    } else if (tc.assertion === 'text') {
      await expect(loc).toHaveText(tc.expected as string);
    } else if (tc.assertion === 'visible+text') {
      await expect(loc).toBeVisible();
      await expect(loc).toContainText(tc.expected as string);
    }
  });
}

// ---------------------------------------------------------------------------
// Data-driven: QMDC definitions hidden
// ---------------------------------------------------------------------------

const hiddenDefCases: {
  name: string;
  fixture: string;
  expectedHiddenCount: number;
  headingSelector?: string;
  expectedVisibleText?: string;
}[] = [
  { name: '[[id: Kind]]', fixture: 'qmdc-id-kind.qmd.md', expectedHiddenCount: 1, headingSelector: 'h2', expectedVisibleText: 'User' },
  { name: '[[id]] without Kind', fixture: 'qmdc-id-only.qmd.md', expectedHiddenCount: 1 },
  { name: '[[:Kind]] auto-ID', fixture: 'qmdc-kind-only.qmd.md', expectedHiddenCount: 1, headingSelector: 'h2', expectedVisibleText: 'Users' },
];

for (const tc of hiddenDefCases) {
  test(`hides ${tc.name} definitions`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const qmdcId = page.locator('.qmdc-id');
    await expect(qmdcId).toHaveCount(tc.expectedHiddenCount);
    await expect(qmdcId.first()).toBeHidden();
    if (tc.headingSelector && tc.expectedVisibleText) {
      const heading = page.locator(tc.headingSelector);
      const innerText = await heading.innerText();
      expect(innerText).toContain(tc.expectedVisibleText);
      expect(innerText).not.toContain('[[');
    }
  });
}

// ---------------------------------------------------------------------------
// Data-driven: QMD.md references render as links
// ---------------------------------------------------------------------------

const refCases: {
  name: string;
  fixture: string;
  expectedCount: number;
  expectedText?: string;
  expectedDataRef?: string;
}[] = [
  { name: 'single [[#ref]]', fixture: 'qmdc-ref-single.qmd.md', expectedCount: 1, expectedText: 'alice', expectedDataRef: 'alice' },
  { name: 'multiple refs', fixture: 'qmdc-ref-multiple.qmd.md', expectedCount: 3 },
  { name: 'cross-namespace ref', fixture: 'qmdc-ref-cross-namespace.qmd.md', expectedCount: 1, expectedText: 'storage:users', expectedDataRef: 'storage:users' },
  { name: 'special chars escaped', fixture: 'qmdc-ref-special-chars.qmd.md', expectedCount: 1, expectedDataRef: 'foo"bar' },
];

for (const tc of refCases) {
  test(`ref: ${tc.name}`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const refs = page.locator('.qmdc-ref');
    await expect(refs).toHaveCount(tc.expectedCount);
    if (tc.expectedText) {
      await expect(refs.first()).toHaveText(tc.expectedText);
    }
    if (tc.expectedDataRef) {
      await expect(refs.first()).toHaveAttribute('data-ref', tc.expectedDataRef);
    }
  });
}

test('ref links have no inline onclick (delegated handler)', async ({ page }) => {
  await renderFixture(page, 'qmdc-ref-special-chars.qmd.md');
  const ref = page.locator('.qmdc-ref');
  await expect(ref).not.toHaveAttribute('onclick');
});

// ---------------------------------------------------------------------------
// Data-driven: heading anchors render cleanly (no raw [[...]] visible)
// ---------------------------------------------------------------------------

const cleanHeadingCases: {
  name: string;
  fixture: string;
  headingSelector: string;
  expectedText: string;
}[] = [
  { name: '[[id: Kind]] heading', fixture: 'heading-id-kind-clean.qmd.md', headingSelector: 'h2', expectedText: 'Payment Service' },
  { name: '[[id]] heading', fixture: 'heading-id-only-clean.qmd.md', headingSelector: 'h2', expectedText: 'Database Config' },
];

for (const tc of cleanHeadingCases) {
  test(`heading clean: ${tc.name}`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const heading = page.locator(tc.headingSelector);
    const innerText = await heading.innerText();
    expect(innerText).toContain(tc.expectedText);
    expect(innerText).not.toContain('[[');
  });
}

test('heading anchor generates correct id attribute', async ({ page }) => {
  await renderFixture(page, 'heading-id-attribute.qmd.md');
  const span = page.locator('.qmdc-id#users');
  await expect(span).toHaveCount(1);
});

test('multiple headings with anchors all render cleanly', async ({ page }) => {
  await renderFixture(page, 'heading-multiple-anchors.qmd.md');
  const h2s = page.locator('h2');
  await expect(h2s).toHaveCount(2);
  expect(await h2s.nth(0).innerText()).toContain('Auth');
  expect(await h2s.nth(0).innerText()).not.toContain('[[');
  expect(await h2s.nth(1).innerText()).toContain('Database');
  expect(await h2s.nth(1).innerText()).not.toContain('[[');
  const h3 = page.locator('h3');
  expect(await h3.innerText()).not.toContain('[[');
});

// ---------------------------------------------------------------------------
// Code fence protection
// ---------------------------------------------------------------------------

test('does not transform [[#ref]] inside code blocks', async ({ page }) => {
  await renderFixture(page, 'code-fence-protects-refs.qmd.md');
  await expect(page.locator('.qmdc-ref')).toHaveCount(0);
  await expect(page.locator('code')).toContainText('[[#should_not_transform]]');
});

test('example code blocks: refs not transformed, content shown as code', async ({ page }) => {
  await renderFixture(page, 'code-fence-example-modifier.qmd.md');
  // The [[#some_object]] in regular content SHOULD be a link
  const refs = page.locator('.qmdc-ref');
  await expect(refs).toHaveCount(1);
  await expect(refs.first()).toHaveText('some_object');

  // Example code blocks should render as <pre><code> — not as rendered HTML
  const codeBlocks = page.locator('pre code');
  // 3 example blocks: markdown, json, sql
  await expect(codeBlocks).toHaveCount(3);

  // [[#user_profile]] inside example blocks must NOT become .qmdc-ref links
  const allRefs = page.locator('.qmdc-ref');
  await expect(allRefs).toHaveCount(1); // only the one in regular content

  // The markdown example block should contain raw [[...]] text, not rendered HTML
  const firstCode = codeBlocks.nth(0);
  await expect(firstCode).toContainText('[[alice: User]]');
  await expect(firstCode).toContainText('[[#user_profile]]');
});

// ---------------------------------------------------------------------------
// Table blocks
// ---------------------------------------------------------------------------

test('table block without executor shows error', async ({ page }) => {
  await renderFixture(page, 'table-block-no-executor.qmd.md');
  const error = page.locator('.error');
  await expect(error).toBeVisible();
  await expect(error).toContainText('Query executor not available');
});

test('table block with executor renders HTML table', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('table-block-no-executor.qmd.md'),
    {
      async executeQuery() {
        return { success: true, columns: ['__id'], rows: [['alice'], ['bob']] };
      },
    },
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });
  const rows = page.locator('tbody tr');
  await expect(rows).toHaveCount(2);
  await expect(rows.first()).toContainText('alice');
});

// ---------------------------------------------------------------------------
// transformQmdcSyntax unit checks
// ---------------------------------------------------------------------------

test('transformQmdcSyntax strips [[id: Kind]] from text', () => {
  const result = transformQmdcSyntax('## User [[alice: User]]');
  expect(result).toContain('<span class="qmdc-id" data-pagefind-ignore id="alice">[[alice: User]]</span>');
  expect(result).toContain('<span class="qmdc-kind" data-pagefind-filter="kind">User</span>');
});

test('transformQmdcSyntax converts [[#ref]] to link', () => {
  const result = transformQmdcSyntax('See [[#alice]] for details.');
  expect(result).toContain('class="qmdc-ref"');
  expect(result).toContain('data-ref="alice"');
  expect(result).toContain('>alice</a>');
});

test('transformQmdcSyntax: 4-backtick wrapper protects nested ``` fence (no placeholder leak, refs untouched)', () => {
  // A 4-backtick wrapper that shows a 3-backtick example block containing a ref.
  // The inner ``` must NOT close the outer fence, the ref must stay raw, and no
  // ___CODE_FENCE___ placeholder may leak into the output.
  const input = [
    '````markdown example',
    '```qmd.md',
    '## User [[alice: User]]',
    '- profile: [[#user_profile]]',
    '```',
    '````',
    '',
    'Then a real ref: [[#some_object]].',
  ].join('\n');
  const result = transformQmdcSyntax(input);
  expect(result).not.toContain('___CODE_FENCE_');
  // Inside the wrapper: refs/ids are left as raw text (protected as code).
  expect(result).toContain('[[#user_profile]]');
  expect(result).toContain('[[alice: User]]');
  expect(result).not.toContain('data-ref="user_profile"');
  // The real ref outside the fence IS transformed.
  expect(result).toContain('data-ref="some_object"');
});

test('rewriteImageSources: local img src goes through the resolver, remote/data left intact', () => {
  const html =
    '<img src="assets/hero.png" alt="h">' +
    '<img src="https://x.test/a.png">' +
    '<img src="data:image/png;base64,AAAA">';
  const out = rewriteImageSources(html, (src) => `vscode-webview://res/${src}`);
  expect(out).toContain('src="vscode-webview://res/assets/hero.png"');
  // remote + data URIs untouched
  expect(out).toContain('src="https://x.test/a.png"');
  expect(out).toContain('src="data:image/png;base64,AAAA"');
});

test('generatePreviewHtml: ![](local) image is resolved to a webview URI', async () => {
  const html = await generatePreviewHtml(
    '# Doc\n\n![hero](assets/hero.png)\n',
    null,
    'file:///ws/docs/page.qmd.md',
    { includeVscodeApi: false, resolveImageSrc: (src) => `vscode-webview://res/${src}` },
  );
  expect(html).toContain('<img');
  expect(html).toContain('vscode-webview://res/assets/hero.png');
  // the raw relative path must not survive as the img src
  expect(html).not.toMatch(/src="assets\/hero\.png"/);
});

// ---------------------------------------------------------------------------
// Data-driven: mermaid diagrams
// ---------------------------------------------------------------------------

const mermaidCases: {
  name: string;
  fixture: string;
  expectedDivCount: number;
  /** Text that must appear inside the mermaid div (not mangled) */
  mustContain?: string[];
  /** Text that must NOT appear in the mermaid div (mangled artifacts) */
  mustNotContain?: string[];
  /** Expected count of <pre> blocks (non-mermaid code blocks) */
  expectedPreCount?: number;
}[] = [
  {
    name: 'simple graph',
    fixture: 'mermaid-simple.qmd.md',
    expectedDivCount: 1,
    mustContain: ['graph TD'],
  },
  {
    name: 'mixed with regular code block',
    fixture: 'mermaid-mixed-with-code.qmd.md',
    expectedDivCount: 1,
    expectedPreCount: 1,
  },
  {
    name: 'complex subgraphs with <br/> and indentation',
    fixture: 'mermaid-complex-subgraphs.qmd.md',
    expectedDivCount: 1,
    mustContain: ['subgraph', '<br>'],
    mustNotContain: ['<pre><code>'],
  },
  {
    name: 'sequenceDiagram with arrows and opt blocks',
    fixture: 'mermaid-sequence-diagram.qmd.md',
    expectedDivCount: 1,
    mustContain: ['sequenceDiagram', 'opt Error occurs'],
    mustNotContain: ['<pre><code>'],
  },
];

for (const tc of mermaidCases) {
  test(`mermaid: ${tc.name}`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const mermaidDivs = page.locator('div.mermaid');
    await expect(mermaidDivs).toHaveCount(tc.expectedDivCount);

    if (tc.mustContain || tc.mustNotContain) {
      const divHtml = await mermaidDivs.first().innerHTML();
      for (const text of tc.mustContain ?? []) {
        expect(divHtml, `expected mermaid div to contain "${text}"`).toContain(text);
      }
      for (const text of tc.mustNotContain ?? []) {
        expect(divHtml, `expected mermaid div NOT to contain "${text}"`).not.toContain(text);
      }
    }

    if (tc.expectedPreCount !== undefined) {
      await expect(page.locator('pre')).toHaveCount(tc.expectedPreCount);
    }

    // No mermaid content should end up in a <code> block
    await expect(page.locator('code.language-mermaid')).toHaveCount(0);
  });
}

test('mermaid renders SVG when mermaid.js is loaded', async ({ page }) => {
  await renderFixture(page, 'mermaid-simple.qmd.md');
  const mermaidPath = path.resolve(__dirname, '..', 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
  await page.addScriptTag({ path: mermaidPath });
  await page.evaluate(() => {
    (window as any).mermaid.initialize({ startOnLoad: false, theme: 'dark' });
    return (window as any).mermaid.run();
  });
  const svg = page.locator('div.mermaid svg');
  await expect(svg).toHaveCount(1);
});

// Regression: complex mermaid diagrams with indentation, <br/>, and arrows
// must survive the markdown pipeline and render as SVG (was crashing before fix)
const mermaidSvgCases = [
  { name: 'subgraphs with <br/> tags', fixture: 'mermaid-complex-subgraphs.qmd.md' },
  { name: 'sequenceDiagram with ->> arrows', fixture: 'mermaid-sequence-diagram.qmd.md' },
];

for (const tc of mermaidSvgCases) {
  test(`mermaid SVG: ${tc.name}`, async ({ page }) => {
    await renderFixture(page, tc.fixture);
    const mermaidPath = path.resolve(__dirname, '..', 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
    await page.addScriptTag({ path: mermaidPath });
    await page.evaluate(() => {
      (window as any).mermaid.initialize({ startOnLoad: false, theme: 'dark' });
      return (window as any).mermaid.run();
    });
    const svg = page.locator('div.mermaid svg');
    await expect(svg).toHaveCount(1);
  });
}

// The shared enhancement core (getMermaidEnhanceScript → templates/qmdc-mermaid-core.js)
// must, after mermaid renders, wrap each diagram in a zoom/pan viewport + toolbar.
test('mermaid enhancement core builds a zoom/pan viewport and toolbar', async ({ page }) => {
  await renderFixture(page, 'mermaid-simple.qmd.md');
  const mermaidPath = path.resolve(__dirname, '..', 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
  await page.addScriptTag({ path: mermaidPath });
  // Inject the SAME script the extension injects into the webview.
  await page.addScriptTag({ content: getMermaidEnhanceScript() });

  // The core runs on load and enhances the diagram. (dataset.zoomReady → the
  // DOM attribute data-zoom-ready.)
  const container = page.locator('div.mermaid');
  await expect(container).toHaveAttribute('data-zoom-ready', '1');
  await expect(page.locator('.mermaid-viewport svg')).toHaveCount(1);
  // Toolbar with the five controls (zoom out / label / zoom in / fit / 1:1).
  await expect(page.locator('.mermaid-toolbar')).toHaveCount(1);
  await expect(page.locator('.mermaid-toolbar .mermaid-btn')).toHaveCount(4);
  // Zooming in updates the percentage label and can make the diagram pannable.
  const zoomIn = page.locator('.mermaid-btn[aria-label="Zoom in (Ctrl/Cmd + scroll)"]');
  await zoomIn.click();
  await expect(page.locator('.mermaid-zoom-label')).not.toHaveText('100%');
});

test('mermaid enhancement core pins securityLevel strict (no script execution)', () => {
  const script = getMermaidEnhanceScript();
  expect(script).toContain("securityLevel: \"strict\"");
  // VS Code host contract: always-dark webview.
  expect(script).toContain("window.__qmdcMermaidTheme = 'dark'");
});

// ---------------------------------------------------------------------------
// Mermaid zoom / pan viewport (large diagrams)
// ---------------------------------------------------------------------------

// Renders a fixture, loads mermaid.min.js, then runs the same enhancement script
// the webview injects (getMermaidEnhanceScript()). Waits for the zoom viewport.
async function renderWithMermaidEnhance(page: import('@playwright/test').Page, name: string) {
  await renderFixture(page, name);
  const mermaidPath = path.resolve(__dirname, '..', 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
  await page.addScriptTag({ path: mermaidPath });
  await page.addScriptTag({ content: getMermaidEnhanceScript() });
  await page.waitForSelector('div.mermaid svg');
  await page.waitForSelector('.mermaid-viewport');
}

test('mermaid: large diagram is not squished (renders at fit-to-width size)', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');

  const info = await page.evaluate(() => {
    const svgEl = document.querySelector('div.mermaid svg') as SVGSVGElement;
    const wrap = document.querySelector('.mermaid-viewport') as HTMLElement;
    return {
      viewBoxWidth: svgEl.viewBox.baseVal.width,
      renderedWidth: parseFloat(svgEl.style.width),
      maxWidth: svgEl.style.maxWidth,
      contentW: wrap.clientWidth,
    };
  });

  // useMaxWidth is disabled so the SVG is explicitly pixel-sized, not auto-shrunk.
  expect(info.maxWidth).toBe('none');
  expect(info.viewBoxWidth).toBeGreaterThan(1000);
  // Default view fits the diagram to the full column width (within ~2px).
  expect(Math.abs(info.renderedWidth - info.contentW)).toBeLessThanOrEqual(2);
});

test('mermaid: diagram in a horizontal-scroll viewport with toolbar', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');

  await expect(page.locator('.mermaid-viewport')).toHaveCount(1);
  await expect(page.locator('.mermaid-viewport > svg')).toHaveCount(1);
  // 4 toolbar buttons: zoom out, zoom in, fit, actual size
  await expect(page.locator('.mermaid-btn')).toHaveCount(4);

  // The viewport scrolls horizontally only; vertically it follows the SVG so the
  // whole diagram is visible (no fixed-height clipping box).
  const overflow = await page.locator('.mermaid-viewport').evaluate(el => {
    const cs = getComputedStyle(el);
    return { x: cs.overflowX, y: cs.overflowY };
  });
  expect(overflow.x).toBe('auto');
  expect(overflow.y).toBe('hidden');
});

test('mermaid: viewport height follows the diagram (no vertical clipping)', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-tall-sequence.qmd.md');

  const info = await page.evaluate(() => {
    const svgEl = document.querySelector('div.mermaid svg') as SVGSVGElement;
    const wrap = document.querySelector('.mermaid-viewport') as HTMLElement;
    return {
      svgHeight: svgEl.getBoundingClientRect().height,
      viewportClientHeight: wrap.clientHeight,
      scrollHeight: wrap.scrollHeight,
    };
  });

  // The viewport is tall enough to show the entire diagram — its visible height
  // matches the rendered SVG height, so nothing is cut off vertically.
  expect(info.viewportClientHeight).toBeGreaterThanOrEqual(info.svgHeight - 2);
  // And there is no hidden vertical overflow to scroll/pan through.
  expect(info.scrollHeight).toBeLessThanOrEqual(info.viewportClientHeight + 2);
});

test('mermaid: zoom buttons resize the SVG, 1:1 = natural size', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');

  const readSize = () => page.evaluate(() => {
    const svg = document.querySelector('div.mermaid svg') as SVGSVGElement;
    return { w: parseFloat(svg.style.width), vb: svg.viewBox.baseVal.width };
  });

  const before = await readSize();
  await page.locator('.mermaid-btn', { hasText: '+' }).click();
  const after = await readSize();
  expect(after.w).toBeGreaterThan(before.w);

  // 1:1 renders at natural pixel size (width == viewBox width).
  await page.locator('.mermaid-btn', { hasText: '1:1' }).click();
  const actual = await readSize();
  expect(actual.w).toBeCloseTo(actual.vb, 0);
});

test('mermaid: zooming in past column width enables horizontal scroll', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-long-labels.qmd.md');

  // Zoom in a few times so the diagram exceeds the column width.
  for (let i = 0; i < 4; i++) {
    await page.locator('.mermaid-btn', { hasText: '+' }).click();
  }
  const scrollable = await page.locator('.mermaid-viewport').evaluate(el => el.scrollWidth > el.clientWidth + 1);
  expect(scrollable).toBe(true);
});

test('mermaid: enhancement is idempotent (single viewport, no double-wrap)', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-simple.qmd.md');
  // Re-run the enhancement script; the data-zoomReady guard must prevent re-wrapping.
  await page.addScriptTag({ content: getMermaidEnhanceScript() });
  await expect(page.locator('.mermaid-viewport')).toHaveCount(1);
  await expect(page.locator('div.mermaid svg')).toHaveCount(1);
});

test('mermaid: long sequence labels wrap onto multiple lines (not one wide line)', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-long-labels.qmd.md');

  const info = await page.evaluate(() => {
    const svg = document.querySelector('div.mermaid svg') as SVGSVGElement;
    // With wrap:true mermaid emits one <text.messageText> per wrapped line.
    const lines = Array.from(svg.querySelectorAll('text.messageText')) as SVGTextElement[];
    return {
      viewBoxWidth: svg.viewBox.baseVal.width,
      messageLines: lines.length,
    };
  });

  // Robust assertion: the single long message is split across multiple lines.
  // (Exact line count / pixel width depend on mermaid version + CI font metrics,
  // so we only assert that wrapping happened.)
  expect(info.messageLines).toBeGreaterThan(1);
  // Sanity bound: a wrapped diagram is much narrower than the ~1700px single-line
  // version. Kept generous to avoid coupling to font metrics.
  expect(info.viewBoxWidth).toBeLessThan(1200);
});

test('mermaid: one broken diagram does not block enhancing valid ones', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderFixture(page, 'mermaid-one-broken.qmd.md');
  const mermaidPath = path.resolve(__dirname, '..', 'node_modules', 'mermaid', 'dist', 'mermaid.min.js');
  await page.addScriptTag({ path: mermaidPath });
  await page.addScriptTag({ content: getMermaidEnhanceScript() });

  // suppressErrors keeps the batch alive; the valid graph must still get a viewport.
  await page.waitForSelector('.mermaid-viewport', { timeout: 5000 });
  const valid = await page.evaluate(() => {
    const vps = document.querySelectorAll('.mermaid-viewport');
    // At least one viewport with a rendered SVG sized to fit (not max-width auto).
    let ok = 0;
    vps.forEach(vp => { const s = vp.querySelector('svg'); if (s && (s as SVGSVGElement).style.maxWidth === 'none') ok++; });
    return ok;
  });
  expect(valid).toBeGreaterThanOrEqual(1);
});

test('mermaid: grab affordance only when the diagram overflows', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });

  // At fit-to-width the diagram exactly fills the column — nothing to pan.
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');
  await expect(page.locator('.mermaid-viewport')).not.toHaveClass(/mermaid-pannable/);

  // One zoom-in pushes it past the column width → pannable affordance appears.
  await page.locator('.mermaid-btn', { hasText: '+' }).click();
  await expect(page.locator('.mermaid-viewport')).toHaveClass(/mermaid-pannable/);
});

test('mermaid: toolbar buttons have accessible names', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');

  await expect(page.getByRole('button', { name: /zoom in/i })).toHaveCount(1);
  await expect(page.getByRole('button', { name: /zoom out/i })).toHaveCount(1);
  await expect(page.getByRole('button', { name: /fit to width/i })).toHaveCount(1);
  await expect(page.getByRole('button', { name: /actual size/i })).toHaveCount(1);
});

test('mermaid: viewport is keyboard-focusable and arrow keys pan', async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  await renderWithMermaidEnhance(page, 'mermaid-large-sequence.qmd.md');

  const vp = page.locator('.mermaid-viewport');
  await expect(vp).toHaveAttribute('tabindex', '0');

  // Zoom in so there is horizontal overflow to scroll, then pan with ArrowRight.
  for (let i = 0; i < 3; i++) await page.locator('.mermaid-btn', { hasText: '+' }).click();
  await vp.focus();
  const before = await vp.evaluate(el => el.scrollLeft);
  await page.keyboard.press('ArrowRight');
  await page.keyboard.press('ArrowRight');
  const after = await vp.evaluate(el => el.scrollLeft);
  expect(after).toBeGreaterThan(before);
});

// ---------------------------------------------------------------------------
// Scroll-to-anchor after navigation
// ---------------------------------------------------------------------------

test('scrollToId option scrolls to target anchor', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('scroll-target.qmd.md'),
    null,
    'file:///test.qmd.md',
    { includeVscodeApi: false, scrollToId: 'third_section' },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // The target span should exist
  const target = page.locator('.qmdc-id#third_section');
  await expect(target).toHaveCount(1);

  // Give the smooth scroll a moment to complete
  await page.waitForTimeout(500);

  // The heading containing the target should be in the viewport
  const heading = page.locator('h2', { has: page.locator('#third_section') });
  await expect(heading).toBeInViewport();
});

test('scrollToId with non-existent id does not crash', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('basic-heading.qmd.md'),
    null,
    'file:///test.qmd.md',
    { includeVscodeApi: false, scrollToId: 'nonexistent' },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });
  // Should render without errors
  await expect(page.locator('h1')).toHaveText('Hello World');
});

// ---------------------------------------------------------------------------
// Scroll-to-anchor on navigation
// ---------------------------------------------------------------------------

test('scrollToId scrolls target heading into view', async ({ page }) => {
  // Use a small viewport so the target is definitely below the fold
  await page.setViewportSize({ width: 800, height: 400 });

  const html = await generatePreviewHtml(
    fixture('scroll-target-long.qmd.md'),
    null,
    'file:///test.qmd.md',
    { includeVscodeApi: false, scrollToId: 'target_obj' },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Wait for smooth scroll to complete
  await page.waitForTimeout(500);

  // The target heading should be in the viewport (or very close to the top)
  const heading = page.locator('#target_obj').locator('..');
  const box = await heading.boundingBox();
  expect(box, 'target heading should have a bounding box').not.toBeNull();
  // The heading's top should be within the viewport (0..400)
  expect(box!.y).toBeGreaterThanOrEqual(-10);
  expect(box!.y).toBeLessThan(400);
});

// ---------------------------------------------------------------------------
// Sidebar: page TOC + graph context
// ---------------------------------------------------------------------------

import { extractPageToc, renderSidebar, fetchGraphContext } from '../src/preview-renderer';

test('extractPageToc extracts h2/h3 headings with IDs', () => {
  const html = `
    <h1>Title</h1>
    <h2><span class="qmdc-id" id="section_a">[[section_a]]</span>Section A</h2>
    <p>content</p>
    <h3><span class="qmdc-id" id="sub_b">[[sub_b]]</span>Sub B</h3>
    <h2><span class="qmdc-id" id="section_c">[[section_c: Kind]]</span>Section C</h2>
  `;
  const toc = extractPageToc(html);
  expect(toc).toHaveLength(3);
  expect(toc[0]).toEqual({ level: 2, id: 'section_a', text: 'Section A' });
  expect(toc[1]).toEqual({ level: 3, id: 'sub_b', text: 'Sub B' });
  expect(toc[2]).toEqual({ level: 2, id: 'section_c', text: 'Section C' });
});

test('sidebar renders page TOC without graph context', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('sidebar-toc.qmd.md'),
    null,
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Should have a sidebar with TOC
  const sidebar = page.locator('.qmdc-sidebar');
  await expect(sidebar).toHaveCount(1);

  // TOC should have links for h2/h3 headings (not h1)
  const tocLinks = page.locator('.sb-toc-link');
  await expect(tocLinks).toHaveCount(4); // rule_broken_link, rule_ambiguous_ref, resolution_strategy, rule_duplicate_id

  // First TOC link should be "Broken Link"
  await expect(tocLinks.first()).toContainText('Broken Link');
});

test('sidebar renders graph context with mock executor', async ({ page }) => {
  const mockExecutor = {
    async executeQuery(sql: string) {
      // File lookup (first query in fetchGraphContext)
      if (sql.includes('__file LIKE')) {
        return {
          success: true,
          columns: ['__file', '__namespace', '__workspace'],
          rows: [['lsp/diagnostics.qmd.md', 'lsp', 'docs']],
        };
      }
      // Workspace label
      if (sql.includes("__kind = '__Workspace'")) {
        return { success: true, columns: ['__label'], rows: [['QMDC Docs']] };
      }
      // Namespace label
      if (sql.includes("__kind = '__Namespace'") && sql.includes("__id = 'lsp'")) {
        return { success: true, columns: ['__label'], rows: [['LSP']] };
      }
      // File label sub-queries (LIMIT 1 with specific file)
      if (sql.includes("__file = '") && sql.includes('LIMIT 1')) {
        if (sql.includes('completion')) {
          return { success: true, columns: ['__label'], rows: [['Completion']] };
        }
        if (sql.includes('diagnostics')) {
          return { success: true, columns: ['__label'], rows: [['Diagnostics']] };
        }
        return { success: true, columns: ['__label'], rows: [] };
      }
      // Siblings (GROUP BY __file, __kind)
      if (sql.includes('GROUP BY __file')) {
        return {
          success: true,
          columns: ['__file', '__kind', 'cnt'],
          rows: [
            ['lsp/completion.qmd.md', 'LSPFeature', 1],
            ['lsp/diagnostics.qmd.md', 'DiagnosticRule', 8],
            ['lsp/diagnostics.qmd.md', 'LSPFeature', 1],
          ],
        };
      }
      // Outgoing edges
      if (sql.includes('edges') && sql.includes('t.__label')) {
        return {
          success: true,
          columns: ['edge_type', '__label', '__kind'],
          rows: [['depends', 'Rust Parser', 'Parser']],
        };
      }
      // Incoming edges
      if (sql.includes('edges') && sql.includes('s.__label')) {
        return {
          success: true,
          columns: ['edge_type', '__label', '__kind'],
          rows: [['about', 'Capabilities', 'NarrativeDoc']],
        };
      }
      return { success: true, columns: [], rows: [] };
    },
  };

  const html = await generatePreviewHtml(
    fixture('sidebar-toc.qmd.md'),
    mockExecutor,
    'file:///workspace/lsp/diagnostics.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Breadcrumb should be present
  const crumbs = page.locator('.sb-crumb');
  await expect(crumbs).toHaveCount(3);
  await expect(crumbs.first()).toContainText('QMDC Docs');

  // Siblings section — the mock returns 2 sibling rows
  const sibs = page.locator('.sb-sib');
  const sibCount = await sibs.count();
  expect(sibCount).toBeGreaterThanOrEqual(2);

  // Current file should be highlighted
  const currentSib = page.locator('.sb-sib--current');
  await expect(currentSib).toHaveCount(1);
  await expect(currentSib).toContainText('Diagnostics');

  // Links to section
  const linksToVerb = page.locator('.sb-edge-verb').first();
  await expect(linksToVerb).toContainText('depends on');

  // Linked from section
  const linkedFromVerb = page.locator('.sb-edge-verb').last();
  await expect(linkedFromVerb).toContainText('described in');
});

test('sidebar is responsive: hamburger on narrow viewport', async ({ page }) => {
  await page.setViewportSize({ width: 600, height: 800 });

  const html = await generatePreviewHtml(
    fixture('sidebar-toc.qmd.md'),
    null,
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Hamburger button should be visible
  const toggle = page.locator('.sb-toggle');
  await expect(toggle).toBeVisible();

  // Sidebar should be off-screen (not visible)
  const sidebar = page.locator('.qmdc-sidebar');
  await expect(sidebar).not.toBeInViewport();

  // Click hamburger — sidebar slides in
  await toggle.click();
  await expect(sidebar).toHaveClass(/sb-open/);
  await expect(sidebar).toBeInViewport();

  // Click close button — sidebar slides out
  const close = page.locator('.sb-close');
  await close.click();
  await expect(sidebar).not.toHaveClass(/sb-open/);
});

// ---------------------------------------------------------------------------
// Search bar
// ---------------------------------------------------------------------------

test('search bar is present and focusable with / key', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('search-objects.qmd.md'),
    {
      async executeQuery(sql: string) {
        if (sql.includes('substr(__kind') && sql.includes('__label IS NOT NULL') && sql.includes('ORDER BY __label')) {
          return {
            success: true,
            columns: ['__id', '__label', '__kind', '__file', '__namespace'],
            rows: [
              ['auth_service', 'Auth Service', 'Service', 'test.qmd.md', ''],
              ['payment_gw', 'Payment Gateway', 'Gateway', 'test.qmd.md', ''],
              ['user_db', 'User Database', 'Database', 'test.qmd.md', ''],
            ],
          };
        }
        return { success: true, columns: [], rows: [] };
      },
    },
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Search input should exist
  const input = page.locator('.qmdc-search-input');
  await expect(input).toHaveCount(1);
  await expect(input).toBeVisible();

  // "/" key should focus the search input
  await page.keyboard.press('/');
  await expect(input).toBeFocused();
});

test('search filters objects and shows results', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('search-objects.qmd.md'),
    {
      async executeQuery(sql: string) {
        if (sql.includes('substr(__kind') && sql.includes('__label IS NOT NULL') && sql.includes('ORDER BY __label')) {
          return {
            success: true,
            columns: ['__id', '__label', '__kind', '__file', '__namespace'],
            rows: [
              ['auth_service', 'Auth Service', 'Service', 'test.qmd.md', ''],
              ['payment_gw', 'Payment Gateway', 'Gateway', 'test.qmd.md', ''],
              ['user_db', 'User Database', 'Database', 'test.qmd.md', ''],
            ],
          };
        }
        return { success: true, columns: [], rows: [] };
      },
    },
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  const input = page.locator('.qmdc-search-input');
  const results = page.locator('.qmdc-search-results');

  // Type "auth" — should show Auth Service
  await input.fill('auth');
  await expect(results).toHaveClass(/visible/);
  const items = page.locator('.qmdc-search-item');
  await expect(items.first()).toContainText('Auth Service');

  // Type "pay" — should show Payment Gateway
  await input.fill('pay');
  await expect(items.first()).toContainText('Payment Gateway');

  // Type "x" (single char) — results should hide (min 2 chars)
  await input.fill('x');
  await expect(results).not.toHaveClass(/visible/);
});

test('search keyboard navigation works', async ({ page }) => {
  const html = await generatePreviewHtml(
    fixture('search-objects.qmd.md'),
    {
      async executeQuery(sql: string) {
        if (sql.includes('substr(__kind') && sql.includes('__label IS NOT NULL') && sql.includes('ORDER BY __label')) {
          return {
            success: true,
            columns: ['__id', '__label', '__kind', '__file', '__namespace'],
            rows: [
              ['auth_service', 'Auth Service', 'Service', 'test.qmd.md', ''],
              ['payment_gw', 'Payment Gateway', 'Gateway', 'test.qmd.md', ''],
              ['user_db', 'User Database', 'Database', 'test.qmd.md', ''],
            ],
          };
        }
        return { success: true, columns: [], rows: [] };
      },
    },
    'file:///test.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  const input = page.locator('.qmdc-search-input');
  await input.fill('se');

  // Arrow down should highlight first item
  await page.keyboard.press('ArrowDown');
  const activeItem = page.locator('.qmdc-search-item.active');
  await expect(activeItem).toHaveCount(1);

  // Escape should close results
  await page.keyboard.press('Escape');
  const results = page.locator('.qmdc-search-results');
  await expect(results).not.toHaveClass(/visible/);
});

// ---------------------------------------------------------------------------
// Pagefind data attributes
// ---------------------------------------------------------------------------

test('data-pagefind-ignore on qmdc-id spans', async ({ page }) => {
  await renderFixture(page, 'pagefind-attributes.qmd.md');

  // All .qmdc-id spans should have data-pagefind-ignore
  const qmdcIds = page.locator('.qmdc-id');
  const count = await qmdcIds.count();
  expect(count).toBeGreaterThan(0);
  for (let i = 0; i < count; i++) {
    await expect(qmdcIds.nth(i)).toHaveAttribute('data-pagefind-ignore', '');
  }
});

test('data-pagefind-filter="kind" on qmdc-kind spans', async ({ page }) => {
  await renderFixture(page, 'pagefind-attributes.qmd.md');

  // .qmdc-kind spans should have data-pagefind-filter="kind"
  const kinds = page.locator('.qmdc-kind');
  const count = await kinds.count();
  expect(count).toBeGreaterThan(0);
  for (let i = 0; i < count; i++) {
    await expect(kinds.nth(i)).toHaveAttribute('data-pagefind-filter', 'kind');
  }
});

test('system types (__Workspace) do not get pagefind-filter', () => {
  const result = transformQmdcSyntax('# My Project [[myproject: __Workspace]]');
  // Should have qmdc-id span (hidden)
  expect(result).toContain('class="qmdc-id"');
  expect(result).toContain('data-pagefind-ignore');
  // Should NOT have a .qmdc-kind span (system types are fully hidden)
  expect(result).not.toContain('qmdc-kind');
  expect(result).not.toContain('data-pagefind-filter');
});

// ---------------------------------------------------------------------------
// Breadcrumb links
// ---------------------------------------------------------------------------

test('breadcrumbs render as links for workspace and namespace', async ({ page }) => {
  const mockExecutor = {
    async executeQuery(sql: string) {
      if (sql.includes('__file LIKE')) {
        return {
          success: true,
          columns: ['__file', '__namespace'],
          rows: [['format/objects.qmd.md', 'format']],
        };
      }
      if (sql.includes("__kind = '__Workspace'")) {
        return { success: true, columns: ['__label', '__file'], rows: [['QMDC Docs', 'readme.qmd.md']] };
      }
      if (sql.includes("__kind = '__Namespace'") && sql.includes("__id = 'format'")) {
        return { success: true, columns: ['__label', '__file'], rows: [['Format', 'format/readme.qmd.md']] };
      }
      if (sql.includes('__level = 1') && sql.includes('LIMIT 1')) {
        return { success: true, columns: ['__label'], rows: [['Object']] };
      }
      if (sql.includes('GROUP BY __file')) {
        return { success: true, columns: ['__file', '__kind', 'cnt'], rows: [] };
      }
      if (sql.includes('edges')) {
        return { success: true, columns: [], rows: [] };
      }
      return { success: true, columns: [], rows: [] };
    },
  };

  const html = await generatePreviewHtml(
    fixture('pagefind-attributes.qmd.md'),
    mockExecutor,
    'file:///workspace/format/objects.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Workspace breadcrumb should be a link
  const wsCrumb = page.locator('a.sb-crumb--workspace');
  await expect(wsCrumb).toHaveCount(1);
  await expect(wsCrumb).toContainText('QMDC Docs');
  await expect(wsCrumb).toHaveAttribute('href', /.+\.html/);

  // Namespace breadcrumb should be a link
  const nsCrumb = page.locator('a.sb-crumb--namespace');
  await expect(nsCrumb).toHaveCount(1);
  await expect(nsCrumb).toContainText('Format');
  await expect(nsCrumb).toHaveAttribute('href', /.+\.html/);

  // File breadcrumb should be a span (not a link — it's the current page)
  const fileCrumb = page.locator('span.sb-crumb--file');
  await expect(fileCrumb).toHaveCount(1);
});

// ---------------------------------------------------------------------------
// Link navigation (click → navigateToRef)
// ---------------------------------------------------------------------------

test('clicking qmdc-ref in content calls navigateToRef', async ({ page }) => {
  await renderFixture(page, 'qmdc-ref-single.qmd.md');

  // Capture navigateToRef calls
  await page.evaluate(() => {
    (window as any).__navCalls = [];
    (window as any).navigateToRef = (refId: string) => {
      (window as any).__navCalls.push(refId);
    };
  });

  const ref = page.locator('.qmdc-ref').first();
  await ref.click();

  const calls = await page.evaluate(() => (window as any).__navCalls);
  expect(calls).toEqual(['alice']);
});

test('clicking sidebar sibling link calls navigateToRef in preview mode', async ({ page }) => {
  const mockExecutor = {
    async executeQuery(sql: string) {
      if (sql.includes('__file LIKE')) {
        return { success: true, columns: ['__file', '__namespace'], rows: [['lsp/diagnostics.qmd.md', 'lsp']] };
      }
      if (sql.includes("__kind = '__Workspace'")) {
        return { success: true, columns: ['__label', '__file', '__id'], rows: [['QMDC Docs', 'readme.qmd.md', 'docs']] };
      }
      if (sql.includes("__kind = '__Namespace'") && sql.includes("__id = 'lsp'")) {
        return { success: true, columns: ['__label', '__file', '__id'], rows: [['LSP', 'lsp/readme.qmd.md', 'lsp']] };
      }
      if (sql.includes('__level = 1') && sql.includes('LIMIT 1')) {
        return { success: true, columns: ['__label'], rows: [['Diagnostics']] };
      }
      if (sql.includes('GROUP BY __file')) {
        return {
          success: true,
          columns: ['__file', '__kind', 'cnt'],
          rows: [
            ['lsp/completion.qmd.md', 'LSPFeature', '1'],
            ['lsp/diagnostics.qmd.md', 'DiagnosticRule', '8'],
          ],
        };
      }
      if (sql.includes('__level = 1') && sql.includes('__label IS NOT NULL') && !sql.includes('LIMIT')) {
        return {
          success: true,
          columns: ['__file', '__label', '__id'],
          rows: [
            ['lsp/completion.qmd.md', 'Completion', 'completion'],
            ['lsp/diagnostics.qmd.md', 'Diagnostics', 'diagnostics'],
          ],
        };
      }
      if (sql.includes('edges') && sql.includes('t.__label')) {
        return { success: true, columns: ['edge_type', '__label', '__kind', '__file', '__id'], rows: [['depends', 'Rust Parser', 'Parser', 'parsers/rust.qmd.md', 'rust_parser']] };
      }
      if (sql.includes('edges') && sql.includes('s.__label')) {
        return { success: true, columns: ['edge_type', '__label', '__kind', '__file', '__id'], rows: [] };
      }
      if (sql.includes('substr(__kind') && sql.includes('ORDER BY __label')) {
        return { success: true, columns: ['__id', '__label', '__kind', '__file', '__namespace'], rows: [] };
      }
      return { success: true, columns: [], rows: [] };
    },
  };

  const html = await generatePreviewHtml(
    fixture('sidebar-toc.qmd.md'),
    mockExecutor,
    'file:///workspace/lsp/diagnostics.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Override navigateToRef to capture calls
  await page.evaluate(() => {
    (window as any).__navCalls = [];
    (window as any).navigateToRef = (refId: string) => {
      (window as any).__navCalls.push(refId);
    };
  });

  // Click a sidebar sibling link
  const sibLink = page.locator('.sb-sib.qmdc-ref').first();
  await expect(sibLink).toHaveCount(1);
  await sibLink.click();

  const calls = await page.evaluate(() => (window as any).__navCalls);
  expect(calls.length).toBe(1);
  // Should navigate to an object ref, not a file path
  expect(calls[0]).not.toContain('.html');
  expect(calls[0]).not.toContain('/');

  // Click an edge link
  const edgeLink = page.locator('.sb-edge-item.qmdc-ref').first();
  await expect(edgeLink).toHaveCount(1);
  await edgeLink.click();

  const calls2 = await page.evaluate(() => (window as any).__navCalls);
  expect(calls2).toContain('rust_parser');
});

test('clicking breadcrumb link calls navigateToRef in preview mode', async ({ page }) => {
  const mockExecutor = {
    async executeQuery(sql: string) {
      if (sql.includes('__file LIKE')) {
        return { success: true, columns: ['__file', '__namespace'], rows: [['format/objects.qmd.md', 'format']] };
      }
      if (sql.includes("__kind = '__Workspace'")) {
        return { success: true, columns: ['__label', '__file', '__id'], rows: [['QMDC Docs', 'readme.qmd.md', 'docs']] };
      }
      if (sql.includes("__kind = '__Namespace'") && sql.includes("__id = 'format'")) {
        return { success: true, columns: ['__label', '__file', '__id'], rows: [['Format', 'format/readme.qmd.md', 'format']] };
      }
      if (sql.includes('__level = 1') && sql.includes('LIMIT 1')) {
        return { success: true, columns: ['__label'], rows: [['Object']] };
      }
      if (sql.includes('GROUP BY __file')) {
        return { success: true, columns: ['__file', '__kind', 'cnt'], rows: [] };
      }
      if (sql.includes('edges')) {
        return { success: true, columns: [], rows: [] };
      }
      if (sql.includes('substr(__kind') && sql.includes('ORDER BY __label')) {
        return { success: true, columns: ['__id', '__label', '__kind', '__file', '__namespace'], rows: [] };
      }
      return { success: true, columns: [], rows: [] };
    },
  };

  const html = await generatePreviewHtml(
    fixture('pagefind-attributes.qmd.md'),
    mockExecutor,
    'file:///workspace/format/objects.qmd.md',
    { includeVscodeApi: false },
  );
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Override navigateToRef
  await page.evaluate(() => {
    (window as any).__navCalls = [];
    (window as any).navigateToRef = (refId: string) => {
      (window as any).__navCalls.push(refId);
    };
  });

  // Click workspace breadcrumb
  const wsCrumb = page.locator('a.sb-crumb--workspace');
  await expect(wsCrumb).toHaveCount(1);
  await wsCrumb.click();

  const calls = await page.evaluate(() => (window as any).__navCalls);
  expect(calls).toContain('docs');

  // Click namespace breadcrumb
  const nsCrumb = page.locator('a.sb-crumb--namespace');
  await expect(nsCrumb).toHaveCount(1);
  await nsCrumb.click();

  const calls2 = await page.evaluate(() => (window as any).__navCalls);
  expect(calls2).toContain('format');
});

// ---------------------------------------------------------------------------
// Content link clicks with full executor (simulating real VS Code preview)
// ---------------------------------------------------------------------------

test('clicking qmdc-ref in content body works with executor present', async ({ page }) => {
  // Simulate a real preview scenario: executor present, refs in content
  const qmd = `# Test [[test_doc]]

## Service [[my_service: Service]]

- depends: [[#auth_module]]
- uses: [[#database]], [[#cache]]

Some text with a reference to [[#auth_module]] inline.
`;

  const mockExecutor = {
    async executeQuery(sql: string) {
      if (sql.includes('substr(__kind') && sql.includes('__label IS NOT NULL') && sql.includes('ORDER BY __label')) {
        return {
          success: true,
          columns: ['__id', '__label', '__kind', '__file', '__namespace'],
          rows: [
            ['auth_module', 'Auth Module', 'Module', 'auth.qmd.md', ''],
            ['cache', 'Cache', 'Service', 'infra.qmd.md', ''],
            ['database', 'Database', 'Service', 'infra.qmd.md', ''],
            ['my_service', 'Service', 'Service', 'test.qmd.md', ''],
          ],
        };
      }
      if (sql.includes('__file LIKE')) {
        return { success: true, columns: ['__file', '__namespace'], rows: [['test.qmd.md', '']] };
      }
      if (sql.includes("__kind = '__Workspace'")) {
        return { success: true, columns: ['__label', '__file', '__id'], rows: [['Test WS', 'readme.qmd.md', 'test_ws']] };
      }
      if (sql.includes('__level = 1') && sql.includes('LIMIT 1')) {
        return { success: true, columns: ['__label'], rows: [['Test Doc']] };
      }
      if (sql.includes('GROUP BY __file')) {
        return { success: true, columns: ['__file', '__kind', 'cnt'], rows: [] };
      }
      if (sql.includes('edges')) {
        return { success: true, columns: ['edge_type', '__label', '__kind', '__file', '__id'], rows: [] };
      }
      return { success: true, columns: [], rows: [] };
    },
  };

  const html = await generatePreviewHtml(qmd, mockExecutor, 'file:///workspace/test.qmd.md', {
    includeVscodeApi: false,
  });
  await page.setContent(html, { waitUntil: 'networkidle' });

  // Verify refs exist in content
  const refs = page.locator('.qmdc-content .qmdc-ref');
  const refCount = await refs.count();
  expect(refCount).toBeGreaterThanOrEqual(3); // auth_module, database, cache (+ inline auth_module)

  // Override navigateToRef
  await page.evaluate(() => {
    (window as any).__navCalls = [];
    (window as any).navigateToRef = (refId: string) => {
      (window as any).__navCalls.push(refId);
    };
  });

  // Click the first ref in content (should be auth_module)
  const firstRef = refs.first();
  const dataRef = await firstRef.getAttribute('data-ref');
  await firstRef.click();

  const calls = await page.evaluate(() => (window as any).__navCalls);
  expect(calls.length).toBe(1);
  expect(calls[0]).toBe(dataRef);

  // Click another ref
  const secondRef = refs.nth(1);
  const dataRef2 = await secondRef.getAttribute('data-ref');
  await secondRef.click();

  const calls2 = await page.evaluate(() => (window as any).__navCalls);
  expect(calls2.length).toBe(2);
  expect(calls2[1]).toBe(dataRef2);
});

test('content refs have href="#" and are not intercepted by browser navigation', async ({ page }) => {
  await renderFixture(page, 'qmdc-ref-multiple.qmd.md');

  // All refs should have href="#"
  const refs = page.locator('.qmdc-ref');
  const count = await refs.count();
  expect(count).toBe(3);

  for (let i = 0; i < count; i++) {
    const href = await refs.nth(i).getAttribute('href');
    expect(href, `ref ${i} should have href="#"`).toBe('#');
  }
});

test('template replacement does not corrupt script with $ characters', async ({ page }) => {
  // This tests that String.replace doesn't mangle $ in content/scripts
  const qmd = `# Test [[test]]

## Price [[price: Field]]

- value: $100
- formula: $1 + $2 = $3
`;

  const html = await generatePreviewHtml(qmd, null, 'file:///test.qmd.md', {
    includeVscodeApi: false,
  });
  await page.setContent(html, { waitUntil: 'networkidle' });

  // navigateToRef should be defined (script not corrupted)
  const hasFunc = await page.evaluate(() => typeof (window as any).navigateToRef === 'function');
  expect(hasFunc).toBe(true);

  // The search function should work too
  const hasSearch = await page.evaluate(() => !!document.querySelector('.qmdc-search-input'));
  expect(hasSearch).toBe(true);
});
