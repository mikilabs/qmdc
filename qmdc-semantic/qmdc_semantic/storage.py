"""SQLite storage for embeddings and inferred edges.

Based on Finding 4: sqlite-vec + FTS5 schema.
"""

import contextlib
import sqlite3
from pathlib import Path
from typing import Any

import numpy as np


class Storage:
    """SQLite storage for QMDC Semantic.

    Stores:
    - chunks: metadata and text
    - vec_chunks_{dim}: vector embeddings (via sqlite-vec)
    - chunks_fts: FTS5 index for keyword search
    - inferred_edges: semantic similarity edges
    """

    SCHEMA_VERSION = 5  # v5: added trigram FTS table for substring search
    SUPPORTED_DIMENSIONS = [768, 1024, 1536, 3072, 4096]

    def __init__(self, workspace_path: Path | str):
        """Initialize storage for a workspace.

        Args:
            workspace_path: Path to workspace directory.
                Database is created at .qmdc-semantic/embeddings.db
        """
        self.workspace_path = Path(workspace_path)
        self.db_dir = self.workspace_path / ".qmdc-semantic"
        self.db_path = self.db_dir / "embeddings.db"

        # Create directory if needed
        self.db_dir.mkdir(parents=True, exist_ok=True)

        # Connect and initialize
        self.conn = sqlite3.connect(str(self.db_path))
        self.conn.row_factory = sqlite3.Row
        self._load_sqlite_vec()
        self._init_schema()

    def _load_sqlite_vec(self):
        """Load sqlite-vec extension."""
        try:
            import sqlite_vec

            self.conn.enable_load_extension(True)
            sqlite_vec.load(self.conn)
            self.conn.enable_load_extension(False)
        except Exception as e:
            raise RuntimeError(f"Failed to load sqlite-vec: {e}") from e

    def _init_schema(self):
        """Initialize database schema."""
        cursor = self.conn.cursor()

        # Check schema version
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='meta'")
        if cursor.fetchone():
            version = self.get_meta("schema_version")
            if version and int(version) > self.SCHEMA_VERSION:
                raise RuntimeError(f"Schema version mismatch: {version} > {self.SCHEMA_VERSION}")
            elif version and int(version) == self.SCHEMA_VERSION:
                pass  # Already up to date
            elif version and int(version) >= 4:
                # Incremental, non-destructive migration (preserves embeddings).
                self._migrate(int(version))
            elif version:
                # Pre-v4 schema: no incremental path, rebuild (requires re-index).
                self._drop_all()
                self._create_schema()
            else:
                self._create_schema()
        else:
            # Create fresh schema
            self._create_schema()

    def _drop_all(self):
        """Drop all tables for schema migration."""
        cursor = self.conn.cursor()
        # Get all table names
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = [row[0] for row in cursor.fetchall()]
        for table in tables:
            if table.startswith("sqlite_"):
                continue
            with contextlib.suppress(Exception):
                cursor.execute(f"DROP TABLE IF EXISTS [{table}]")
        # Get all virtual tables
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND sql LIKE '%VIRTUAL%'")
        for row in cursor.fetchall():
            with contextlib.suppress(Exception):
                cursor.execute(f"DROP TABLE IF EXISTS [{row[0]}]")
        self.conn.commit()

    def _create_sync_triggers(self, cursor):
        """Create (or replace) the FTS5 sync triggers for the chunks table.

        Shared by fresh schema creation and migrations. Drops any existing
        triggers first so the definition can be updated in place (e.g. to add
        trigram sync) without losing data.
        """
        cursor.execute("DROP TRIGGER IF EXISTS chunks_ai")
        cursor.execute("DROP TRIGGER IF EXISTS chunks_ad")
        cursor.execute("DROP TRIGGER IF EXISTS chunks_au")

        cursor.execute("""
            CREATE TRIGGER chunks_ai AFTER INSERT ON chunks BEGIN
                INSERT INTO chunks_fts(rowid, chunk_id, object_id, text)
                VALUES (new.id, new.chunk_id, new.object_id, new.text);
                INSERT INTO chunks_trigram(rowid, chunk_id, text)
                VALUES (new.id, new.chunk_id, new.text);
            END
        """)

        cursor.execute("""
            CREATE TRIGGER chunks_ad AFTER DELETE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, chunk_id, object_id, text)
                VALUES('delete', old.id, old.chunk_id, old.object_id, old.text);
                INSERT INTO chunks_trigram(chunks_trigram, rowid, chunk_id, text)
                VALUES('delete', old.id, old.chunk_id, old.text);
            END
        """)

        cursor.execute("""
            CREATE TRIGGER chunks_au AFTER UPDATE ON chunks BEGIN
                INSERT INTO chunks_fts(chunks_fts, rowid, chunk_id, object_id, text)
                VALUES('delete', old.id, old.chunk_id, old.object_id, old.text);
                INSERT INTO chunks_fts(rowid, chunk_id, object_id, text)
                VALUES (new.id, new.chunk_id, new.object_id, new.text);
                INSERT INTO chunks_trigram(chunks_trigram, rowid, chunk_id, text)
                VALUES('delete', old.id, old.chunk_id, old.text);
                INSERT INTO chunks_trigram(rowid, chunk_id, text)
                VALUES (new.id, new.chunk_id, new.text);
            END
        """)

    def _migrate(self, from_version: int):
        """Apply incremental, non-destructive migrations up to SCHEMA_VERSION.

        Preserves existing chunks, embeddings, and edges. Each step only adds
        what the new schema version introduced, so an existing index never has
        to be fully re-embedded just to pick up a schema bump.
        """
        cursor = self.conn.cursor()
        version = from_version

        if version < 5:
            # v5 added the trigram FTS table + trigram-aware sync triggers.
            self._migrate_v4_to_v5(cursor)
            version = 5

        self.set_meta("schema_version", str(self.SCHEMA_VERSION))
        self.conn.commit()

    def _migrate_v4_to_v5(self, cursor):
        """v4 -> v5: add trigram substring search without dropping data.

        Creates the chunks_trigram FTS5 table, updates the sync triggers to
        keep it in sync, and backfills it from the existing chunks.
        """
        cursor.execute("""
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_trigram USING fts5(
                chunk_id,
                text,
                content=chunks,
                content_rowid=id,
                tokenize='trigram'
            )
        """)
        # Update triggers so future writes also sync the trigram table.
        self._create_sync_triggers(cursor)
        # Backfill the trigram index from existing chunks (external-content FTS5).
        cursor.execute(
            "INSERT INTO chunks_trigram(rowid, chunk_id, text) "
            "SELECT id, chunk_id, text FROM chunks"
        )

    def _create_schema(self):
        """Create database schema."""
        cursor = self.conn.cursor()

        # Metadata table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT
            )
        """)

        # Chunks table - id INTEGER for FTS5 rowid sync
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chunk_id TEXT UNIQUE NOT NULL,
                object_id TEXT NOT NULL,
                object_kind TEXT,
                chunk_type TEXT,
                source_file TEXT,
                text TEXT,
                text_hash TEXT,
                model_id TEXT,
                parent_chunk_id TEXT,
                embedded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_chunks_chunk_id ON chunks(chunk_id)")

        cursor.execute("CREATE INDEX IF NOT EXISTS idx_chunks_object_id ON chunks(object_id)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_chunks_text_hash ON chunks(text_hash)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_chunks_model_id ON chunks(model_id)")

        # FTS5 for keyword search - includes chunk_id and object_id for ID search
        cursor.execute("""
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
                chunk_id,
                object_id,
                text,
                content=chunks,
                content_rowid=id
            )
        """)

        # Trigram FTS5 for substring matching (finds "333" inside "me333")
        cursor.execute("""
            CREATE VIRTUAL TABLE IF NOT EXISTS chunks_trigram USING fts5(
                chunk_id,
                text,
                content=chunks,
                content_rowid=id,
                tokenize='trigram'
            )
        """)

        # Triggers for FTS5 sync - using id column
        self._create_sync_triggers(cursor)

        # Inferred edges table (source_id, target_id are __global_id format)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS inferred_edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                similarity REAL,
                PRIMARY KEY (source_id, target_id)
            )
        """)

        cursor.execute(
            "CREATE INDEX IF NOT EXISTS idx_inferred_source ON inferred_edges(source_id)"
        )
        cursor.execute(
            "CREATE INDEX IF NOT EXISTS idx_inferred_target ON inferred_edges(target_id)"
        )

        # Explicit edges table (from [[#ref]] in QMD.md files)
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS edges (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                source_field TEXT,
                PRIMARY KEY (source_id, target_id, source_field)
            )
        """)

        cursor.execute("CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id)")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id)")

        # Set schema version
        self.set_meta("schema_version", str(self.SCHEMA_VERSION))
        self.conn.commit()

    def _ensure_vec_table(self, dimension: int):
        """Ensure vec0 table exists for given dimension."""
        # Round up to nearest supported dimension
        target_dim = None
        for dim in self.SUPPORTED_DIMENSIONS:
            if dim >= dimension:
                target_dim = dim
                break
        if target_dim is None:
            target_dim = self.SUPPORTED_DIMENSIONS[-1]

        table_name = f"vec_chunks_{target_dim}"

        # Check if exists
        cursor = self.conn.cursor()
        cursor.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name=?",
            (table_name,),
        )
        if not cursor.fetchone():
            # Create vec0 table
            cursor.execute(f"""
                CREATE VIRTUAL TABLE {table_name} USING vec0(
                    chunk_id TEXT PRIMARY KEY,
                    embedding float[{target_dim}] distance_metric=cosine
                )
            """)
            self.conn.commit()

        return table_name, target_dim

    def get_meta(self, key: str) -> str | None:
        """Get metadata value."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT value FROM meta WHERE key = ?", (key,))
        row = cursor.fetchone()
        return row[0] if row else None

    def set_meta(self, key: str, value: str):
        """Set metadata value."""
        cursor = self.conn.cursor()
        cursor.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?, ?)",
            (key, value),
        )
        self.conn.commit()

    def get_chunk_hashes(self) -> dict[str, str]:
        """Get mapping of chunk_id -> text_hash for incremental updates."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT chunk_id, text_hash FROM chunks")
        return {row["chunk_id"]: row["text_hash"] for row in cursor.fetchall()}

    def compute_diff(
        self,
        chunks: list[dict[str, Any]],
        force: bool = False,
    ) -> dict[str, list[dict[str, Any]]]:
        """Compute diff between workspace chunks and stored chunks.

        Args:
            chunks: List of chunk dicts from workspace.
            force: If True, treat all chunks as new.

        Returns:
            Dict with keys: new, changed, unchanged, deleted
        """
        if force:
            return {
                "new": chunks,
                "changed": [],
                "unchanged": [],
                "deleted": [],
            }

        stored_hashes = self.get_chunk_hashes()
        workspace_ids = {c["chunk_id"] for c in chunks}

        new_chunks = []
        changed_chunks = []
        unchanged_chunks = []

        for chunk in chunks:
            chunk_id = chunk["chunk_id"]
            if chunk_id not in stored_hashes:
                new_chunks.append(chunk)
            elif stored_hashes[chunk_id] != chunk["text_hash"]:
                changed_chunks.append(chunk)
            else:
                unchanged_chunks.append(chunk)

        # Find deleted
        deleted_ids = set(stored_hashes.keys()) - workspace_ids
        deleted_chunks = [{"chunk_id": cid} for cid in deleted_ids]

        return {
            "new": new_chunks,
            "changed": changed_chunks,
            "unchanged": unchanged_chunks,
            "deleted": deleted_chunks,
        }

    def save_chunks(self, chunks: list[dict[str, Any]]):
        """Save chunks with embeddings to storage.

        Args:
            chunks: List of chunk dicts with 'embedding' key.
        """
        if not chunks:
            return

        cursor = self.conn.cursor()

        # Determine dimension from first chunk with embedding
        dimension = None
        for chunk in chunks:
            if "embedding" in chunk:
                dimension = len(chunk["embedding"])
                break

        if dimension is None:
            # No embeddings, just save metadata
            for chunk in chunks:
                cursor.execute(
                    """
                    INSERT OR REPLACE INTO chunks
                    (chunk_id, object_id, object_kind, chunk_type, source_file,
                     text, text_hash, model_id, parent_chunk_id)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                    """,
                    (
                        chunk["chunk_id"],
                        chunk["object_id"],
                        chunk.get("object_kind"),
                        chunk.get("chunk_type"),
                        chunk.get("source_file"),
                        chunk.get("text"),
                        chunk.get("text_hash"),
                        chunk.get("model_id"),
                        chunk.get("parent_chunk_id"),
                    ),
                )
            self.conn.commit()
            return

        # Ensure vec table exists
        vec_table, target_dim = self._ensure_vec_table(dimension)

        for chunk in chunks:
            # Save metadata
            cursor.execute(
                """
                INSERT OR REPLACE INTO chunks
                (chunk_id, object_id, object_kind, chunk_type, source_file,
                 text, text_hash, model_id, parent_chunk_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    chunk["chunk_id"],
                    chunk["object_id"],
                    chunk.get("object_kind"),
                    chunk.get("chunk_type"),
                    chunk.get("source_file"),
                    chunk.get("text"),
                    chunk.get("text_hash"),
                    chunk.get("model_id"),
                    chunk.get("parent_chunk_id"),
                ),
            )

            # Save embedding
            if "embedding" in chunk:
                embedding = chunk["embedding"]
                # Pad to target dimension if needed
                if len(embedding) < target_dim:
                    embedding = np.pad(embedding, (0, target_dim - len(embedding)))

                # Convert to bytes for sqlite-vec
                embedding_bytes = np.array(embedding, dtype=np.float32).tobytes()

                # vec0 doesn't support INSERT OR REPLACE, so delete first
                cursor.execute(
                    f"DELETE FROM {vec_table} WHERE chunk_id = ?",
                    (chunk["chunk_id"],),
                )
                cursor.execute(
                    f"INSERT INTO {vec_table} (chunk_id, embedding) VALUES (?, ?)",
                    (chunk["chunk_id"], embedding_bytes),
                )

        self.conn.commit()

    def delete_chunks(self, chunk_ids: list[str]):
        """Delete chunks by IDs."""
        if not chunk_ids:
            return

        cursor = self.conn.cursor()
        placeholders = ",".join("?" * len(chunk_ids))

        # Delete from chunks table (FTS5 trigger will handle cleanup)
        cursor.execute(f"DELETE FROM chunks WHERE chunk_id IN ({placeholders})", chunk_ids)

        # Delete from all vec tables (vec0 doesn't support IN clause)
        cursor.execute(
            "SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vec_chunks_%'"
        )
        for row in cursor.fetchall():
            table_name = row[0]
            for cid in chunk_ids:
                with contextlib.suppress(Exception):
                    cursor.execute(f"DELETE FROM {table_name} WHERE chunk_id = ?", (cid,))

        # Delete from inferred_edges
        cursor.execute(
            f"DELETE FROM inferred_edges WHERE source_id IN ({placeholders}) "
            f"OR target_id IN ({placeholders})",
            chunk_ids + chunk_ids,
        )

        self.conn.commit()

    def get_chunk(self, chunk_id: str) -> dict[str, Any] | None:
        """Get a single chunk by ID."""
        cursor = self.conn.cursor()
        cursor.execute("SELECT * FROM chunks WHERE chunk_id = ?", (chunk_id,))
        row = cursor.fetchone()
        return dict(row) if row else None

    def get_all_chunks(self, model_id: str | None = None) -> list[dict[str, Any]]:
        """Get all chunks, optionally filtered by model_id."""
        cursor = self.conn.cursor()
        if model_id:
            cursor.execute("SELECT * FROM chunks WHERE model_id = ?", (model_id,))
        else:
            cursor.execute("SELECT * FROM chunks")
        return [dict(row) for row in cursor.fetchall()]

    def knn_search(
        self,
        query_embedding: np.ndarray,
        k: int = 10,
        model_id: str | None = None,
    ) -> list[tuple[str, float]]:
        """KNN search using vec0.

        Args:
            query_embedding: Query vector.
            k: Number of results.
            model_id: Optional filter by model.

        Returns:
            List of (chunk_id, distance) tuples.
        """
        dimension = len(query_embedding)
        vec_table, target_dim = self._ensure_vec_table(dimension)

        # Pad query if needed
        if dimension < target_dim:
            query_embedding = np.pad(query_embedding, (0, target_dim - dimension))

        query_bytes = np.array(query_embedding, dtype=np.float32).tobytes()

        cursor = self.conn.cursor()

        if model_id:
            # Join with chunks table for model filter
            cursor.execute(
                f"""
                SELECT v.chunk_id, v.distance
                FROM {vec_table} v
                JOIN chunks c ON v.chunk_id = c.chunk_id
                WHERE v.embedding MATCH ? AND k = ? AND c.model_id = ?
                ORDER BY v.distance
                """,
                (query_bytes, k, model_id),
            )
        else:
            cursor.execute(
                f"""
                SELECT chunk_id, distance
                FROM {vec_table}
                WHERE embedding MATCH ? AND k = ?
                ORDER BY distance
                """,
                (query_bytes, k),
            )

        return [(row[0], row[1]) for row in cursor.fetchall()]

    def _normalize_fts_query(self, query: str) -> str:
        """Normalize query for FTS5.

        - Converts "QMD-17" to "qmd17 OR QMDC 17" for better matching
        - Handles common ID patterns
        """
        import re

        # Find patterns like QMD-17, TASK-123, etc.
        id_pattern = re.compile(r"\b([A-Za-z]+)-(\d+)\b")

        def replace_id(match):
            prefix = match.group(1).lower()
            num = match.group(2)
            # Return both concatenated and separated forms
            return f"({prefix}{num} OR {match.group(0).replace('-', ' ')})"

        normalized = id_pattern.sub(replace_id, query)
        return normalized

    def fts_search(self, query: str, limit: int = 50) -> list[tuple[str, float]]:
        """FTS5 keyword search.

        Args:
            query: Search query (FTS5 syntax).
            limit: Max results.

        Returns:
            List of (chunk_id, bm25_score) tuples.
        """
        cursor = self.conn.cursor()
        normalized_query = self._normalize_fts_query(query)
        cursor.execute(
            """
            SELECT c.chunk_id, bm25(chunks_fts) as score
            FROM chunks_fts f
            JOIN chunks c ON f.rowid = c.id
            WHERE chunks_fts MATCH ?
            ORDER BY score
            LIMIT ?
            """,
            (normalized_query, limit),
        )
        return [(row[0], row[1]) for row in cursor.fetchall()]

    def trigram_search(self, query: str, limit: int = 50) -> list[tuple[str, float]]:
        """Trigram substring search using FTS5 trigram tokenizer.

        Finds substrings within tokens (e.g., "333" inside "me333").

        Args:
            query: Search query (plain text, will be matched as substring).
            limit: Max results.

        Returns:
            List of (chunk_id, bm25_score) tuples.
        """
        cursor = self.conn.cursor()
        # Trigram tokenizer uses plain text matching (substring)
        # Quote the query to treat it as a literal phrase
        escaped_query = '"' + query.replace('"', '""') + '"'
        try:
            cursor.execute(
                """
                SELECT c.chunk_id, bm25(chunks_trigram) as score
                FROM chunks_trigram t
                JOIN chunks c ON t.rowid = c.id
                WHERE chunks_trigram MATCH ?
                ORDER BY score
                LIMIT ?
                """,
                (escaped_query, limit),
            )
            return [(row[0], row[1]) for row in cursor.fetchall()]
        except Exception:
            return []

    def save_inferred_edges(self, edges: list[tuple[str, str, float]]):
        """Save inferred edges.

        Args:
            edges: List of (source_id, target_id, similarity) tuples.
                   IDs are in __global_id format.
        """
        cursor = self.conn.cursor()
        cursor.executemany(
            """INSERT OR REPLACE INTO inferred_edges
            (source_id, target_id, similarity) VALUES (?, ?, ?)""",
            edges,
        )
        self.conn.commit()

    def save_explicit_edges(self, edges: list[tuple[str, str, str]]):
        """Save explicit edges from [[#ref]] references.

        Args:
            edges: List of (source_id, target_id, source_field) tuples.
                   IDs are in __global_id format.
        """
        cursor = self.conn.cursor()
        # Clear old edges first
        cursor.execute("DELETE FROM edges")
        cursor.executemany(
            "INSERT OR IGNORE INTO edges (source_id, target_id, source_field) VALUES (?, ?, ?)",
            edges,
        )
        self.conn.commit()

    def get_inferred_edges(
        self,
        threshold: float = 0.7,
    ) -> list[tuple[str, str, float]]:
        """Get inferred edges above threshold.

        Returns:
            List of (source_id, target_id, similarity) tuples.
        """
        cursor = self.conn.cursor()
        cursor.execute(
            "SELECT source_id, target_id, similarity FROM inferred_edges WHERE similarity >= ?",
            (threshold,),
        )
        return [(row[0], row[1], row[2]) for row in cursor.fetchall()]

    def get_neighbors(self, object_id: str) -> list[tuple[str, float, str]]:
        """Get neighbors of an object (both explicit and inferred edges).

        Args:
            object_id: Object ID in __global_id format.

        Returns:
            List of (neighbor_id, weight, edge_type) tuples.
        """
        cursor = self.conn.cursor()

        # Explicit edges (both directions) - weight 1.0
        cursor.execute(
            """
            SELECT target_id as neighbor, 1.0 as weight, 'explicit' as type
            FROM edges WHERE source_id = ?
            UNION
            SELECT source_id as neighbor, 1.0 as weight, 'explicit' as type
            FROM edges WHERE target_id = ?
            UNION
            SELECT target_id as neighbor, similarity as weight, 'inferred' as type
            FROM inferred_edges WHERE source_id = ?
            UNION
            SELECT source_id as neighbor, similarity as weight, 'inferred' as type
            FROM inferred_edges WHERE target_id = ?
            """,
            (object_id, object_id, object_id, object_id),
        )

        return [(row[0], row[1], row[2]) for row in cursor.fetchall()]

    def close(self):
        """Close database connection."""
        self.conn.close()
