import { QmdcDatabase } from '../src/db.js';
import { parseWorkspace } from '../src/workspace.js';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ws_path = join(__dirname, '../../tests/workspace/test-workspace');
const result = parseWorkspace(ws_path);
const services = result.objects.find((o) => o.__id === 'services');
const doc = result.objects.find((o) => o.__id === 'doc_ry4ljv');
console.log('services object:', JSON.stringify(services, null, 2));
console.log('\ndoc_ry4ljv object:', JSON.stringify(doc, null, 2));
const db = await QmdcDatabase.create();
db.syncObjects(result.objects);
const edges_services = db.query(
  'SELECT source_id, source_field, target_id FROM edges WHERE source_id = "services" ORDER BY source_id, source_field, target_id'
);
const edges_doc = db.query(
  'SELECT source_id, source_field, target_id FROM edges WHERE source_id = "doc_ry4ljv" ORDER BY source_id, source_field, target_id'
);
console.log(`\nEdges from services: ${edges_services.rows.length}`);
edges_services.rows.forEach((r) => console.log(`${r[0]}|${r[1]}|${r[2]}`));
console.log(`\nEdges from doc_ry4ljv: ${edges_doc.rows.length}`);
edges_doc.rows.forEach((r) => console.log(`${r[0]}|${r[1]}|${r[2]}`));
for (let i = 1; i <= 4; i++) {
  const edges = db.query(
    `SELECT source_id, source_field, target_id FROM edges WHERE source_id = 'text_${i}' ORDER BY target_id`
  );
  console.log(`\ntext_${i}: ${edges.rows.length} edges`);
  edges.rows.forEach((r) => console.log(`  ${r[0]}|${r[1]}|${r[2]}`));
}
const all_edges = db.query(
  'SELECT source_id, source_field, target_id FROM edges ORDER BY source_id, source_field, target_id'
);
console.log(`\nTotal edges: ${all_edges.rows.length}`);
