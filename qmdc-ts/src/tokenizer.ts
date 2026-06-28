/**
 * Markdown tokenizer wrapper using markdown-it
 */

import MarkdownIt from 'markdown-it';
import type Token from 'markdown-it/lib/token';

export function createTokenizer(): MarkdownIt {
  return new MarkdownIt({
    html: true,
  }).enable('table');
}

export function tokenize(markdown: string): Token[] {
  const md: MarkdownIt = createTokenizer();
  return md.parse(markdown, {});
}
