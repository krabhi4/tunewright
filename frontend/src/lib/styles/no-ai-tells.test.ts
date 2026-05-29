import { describe, it, expect } from 'vitest';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { join } from 'node:path';

// Scan both source and shipped static assets (e.g. favicon.svg).
const SRC = fileURLToPath(new URL('../../', import.meta.url)); // frontend/src
const STATIC = fileURLToPath(new URL('../../../static', import.meta.url)); // frontend/static

function walk(dir: string): string[] {
	if (!existsSync(dir)) return [];
	return readdirSync(dir).flatMap((name) => {
		const p = join(dir, name);
		if (statSync(p).isDirectory()) return walk(p);
		return /\.(svelte|css|ts|html|svg)$/.test(p) && !p.endsWith('.test.ts') ? [p] : [];
	});
}

const files = [...walk(SRC), ...walk(STATIC)];
const corpus = files.map((f) => ({ f, text: readFileSync(f, 'utf8') }));

// fonts and palettes that signal "AI default" or are the old Sage & Stone scheme
const FORBIDDEN = [
	'Plus Jakarta',
	'Space Grotesk',
	'JetBrains Mono',
	'Sage & Stone',
	'#4ea388', // old sage accent
	'#34d399', // tailwind emerald-400
	'#fbbf24', // tailwind amber-400
	'#ef4444', // tailwind red-500
	'backdrop-filter'
];

describe('no AI-slop tells', () => {
	for (const needle of FORBIDDEN) {
		it(`no source file contains "${needle}"`, () => {
			const hits = corpus.filter((c) => c.text.includes(needle)).map((c) => c.f);
			expect(hits, `found in:\n${hits.join('\n')}`).toEqual([]);
		});
	}
});
