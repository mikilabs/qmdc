/**
 * Data-driven check of the `.qmdcignore` matcher against git's own answers (QMD-73).
 *
 * Reads `tests/ignore/gitignore-matrix.json`, whose `ignored` lists were produced by git
 * (`tests/ignore/gen_matrix.py`). The Python and Rust parsers run the same cases through
 * their ports of the same matcher, so the `ignore` suite has one case per matrix entry in
 * every language.
 */

import { readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { isIgnoredRelative, parseQmdcignore } from '../src/ignore.js';
import { CaseReport } from './_report.js';

interface MatrixCase {
  name: string;
  content: string;
  ignored: string[];
}

const __dirname = dirname(fileURLToPath(import.meta.url));
const MATRIX = join(__dirname, '../../tests/ignore/gitignore-matrix.json');
const doc = JSON.parse(readFileSync(MATRIX, 'utf-8')) as { tree: string[]; cases: MatrixCase[] };
const encoder = new TextEncoder();
const report = new CaseReport('ts-ignore');

for (const c of doc.cases) {
  report.time(c.name, () => {
    const rules = parseQmdcignore(encoder.encode(c.content));
    const got = doc.tree.filter((p) => isIgnoredRelative(rules, p, false)).sort();
    const want = [...c.ignored].sort();
    if (JSON.stringify(got) !== JSON.stringify(want)) {
      throw new Error(`ignored ${JSON.stringify(got)}, git ignores ${JSON.stringify(want)}`);
    }
  });
}

report.finish();
