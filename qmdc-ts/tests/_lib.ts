/**
 * The library under test, loaded through its PUBLIC entry point only.
 *
 * QMDC_TEST_TARGET=source (default): the in-tree entry `src/index.ts` — the same
 *   module the package's `exports` points at once built, so a name missing from
 *   the public surface fails here too, not only after publishing.
 * QMDC_TEST_TARGET=installed: the packed tarball installed into
 *   QMDC_TEST_INSTALL_DIR (see scripts/package-e2e.sh). It is imported through a
 *   shim inside that directory, so Node resolves `@qmdc/qmdc` exactly as a user's
 *   project would: node_modules lookup plus the package's `exports` map.
 *
 * A switch that silently falls back to the sources would turn the whole
 * installed run green for the wrong reason, so installed mode first proves the
 * module really came from the install directory and aborts otherwise.
 */
import { resolve } from 'path';
import { pathToFileURL, fileURLToPath } from 'url';

type Lib = typeof import('../src/index.js');

async function load(): Promise<Lib> {
  const target = process.env.QMDC_TEST_TARGET ?? 'source';
  if (target === 'source') return import('../src/index.js');
  if (target !== 'installed') {
    throw new Error(`QMDC_TEST_TARGET must be 'source' or 'installed', got '${target}'`);
  }
  const dir = process.env.QMDC_TEST_INSTALL_DIR;
  if (!dir) throw new Error('QMDC_TEST_TARGET=installed needs QMDC_TEST_INSTALL_DIR');
  const shim = (await import(pathToFileURL(resolve(dir, 'entry.mjs')).href)) as Lib & {
    resolvedFrom: string;
  };
  const from = fileURLToPath(shim.resolvedFrom);
  const expected = resolve(dir, 'node_modules', '@qmdc', 'qmdc') + '/';
  if (!from.startsWith(expected)) {
    throw new Error(`@qmdc/qmdc resolved to ${from}, not the install under ${expected}`);
  }
  console.log(`[qmdc under test] installed package: ${from}`);
  return shim;
}

export const lib: Lib = await load();
