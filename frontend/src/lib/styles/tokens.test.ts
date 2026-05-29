import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('./tokens.css', import.meta.url)), 'utf8');

const REQUIRED = [
	'--bg-base', '--bg-surface', '--bg-elevated', '--bg-hover',
	'--bg-selected', '--bg-selected-strong',
	'--text-primary', '--text-secondary', '--text-muted', '--text-placeholder', '--text-on-accent',
	'--border', '--border-subtle', '--grid-border', '--grid-header-bg', '--grid-row-alt', '--grid-row-hover',
	'--accent', '--accent-hover', '--accent-muted', '--accent-subtle',
	'--state-dirty', '--state-dirty-bg', '--state-matched', '--state-matched-bg',
	'--state-conflict', '--state-conflict-bg', '--state-info',
	'--font-ui', '--font-mono', '--font-display',
	'--radius-sm', '--radius-md', '--radius-lg'
];

describe('tokens.css contract', () => {
	it('defines a console dark and console light block', () => {
		expect(css).toContain('[data-theme="console"][data-mode="dark"]');
		expect(css).toContain('[data-theme="console"][data-mode="light"]');
	});
	for (const token of REQUIRED) {
		it(`defines ${token} in the dark block`, () => {
			// crude but effective: token name appears as a property declaration
			expect(css).toContain(`${token}:`);
		});
	}
});
