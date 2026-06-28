/**
 * BlockTree - raw source storage with line<->offset conversion.
 *
 * Used for extracting raw markdown slices without parsing internal structure.
 * Port from Python/Rust implementations.
 */

export class BlockTree {
  readonly source: string;
  private readonly lineStarts: number[];

  constructor(source: string) {
    this.source = source;
    // lineStarts[i] = byte offset where line i begins (0-based)
    this.lineStarts = [0];
    for (let i = 0; i < source.length; i++) {
      if (source[i] === '\n') {
        this.lineStarts.push(i + 1);
      }
    }
  }

  /**
   * Convert 0-based line number to byte offset.
   */
  lineToOffset(line: number): number {
    if (line < 0) return 0;
    if (line >= this.lineStarts.length) return this.source.length;
    return this.lineStarts[line]!;
  }

  /**
   * Get the total number of lines in the source.
   */
  get lineCount(): number {
    return this.lineStarts.length;
  }

  /**
   * Get raw content from startLine (inclusive) to endLine (exclusive), 0-based.
   * Returns the raw slice without any transformation.
   */
  getLinesRaw(startLine: number, endLine: number): string {
    const startOffset = this.lineToOffset(startLine);
    const endOffset = this.lineToOffset(endLine);
    return this.source.slice(startOffset, endOffset);
  }
}
