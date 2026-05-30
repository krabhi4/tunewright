import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('./tokens.css', import.meta.url)), 'utf8');

// The full token contract every theme block must define, so no component
// resolves an undefined custom property when a theme is active.
const REQUIRED = [
	'--bg-base', '--bg-surface', '--bg-elevated', '--bg-hover',
	'--bg-selected', '--bg-selected-strong',
	'--text-primary', '--text-secondary', '--text-muted', '--text-placeholder', '--text-on-accent',
	'--border', '--border-subtle', '--grid-border', '--grid-header-bg', '--grid-row-alt', '--grid-row-hover',
	'--accent', '--accent-hover', '--accent-muted', '--accent-subtle',
	'--state-dirty', '--state-dirty-bg', '--state-matched', '--state-matched-bg',
	'--state-conflict', '--state-conflict-bg', '--state-info',
	'--modified', '--success', '--warning', '--error', '--accent-15', '--error-10',
	'--backdrop', '--shadow-modal', '--shadow-dropdown', '--shadow-context', '--shadow-inset',
	'--font-ui', '--font-mono', '--font-display',
	'--radius-sm', '--radius-md', '--radius-lg'
];

// Console + Editorial support light + dark; Terminal + DAW are dark-native
// (single-attribute selector, no mode requirement).
const BLOCKS = [
	'[data-theme="console"][data-mode="dark"]',
	'[data-theme="console"][data-mode="light"]',
	'[data-theme="editorial"][data-mode="dark"]',
	'[data-theme="editorial"][data-mode="light"]',
	'[data-theme="terminal"]',
	'[data-theme="daw"]'
];

function blockBody(selector: string): string {
	const at = css.indexOf(selector);
	if (at < 0) return '';
	const open = css.indexOf('{', at);
	const close = css.indexOf('}', open);
	if (open < 0 || close < 0) return '';
	return css.slice(open + 1, close);
}

describe('tokens.css contract', () => {
	it('defines theme-invariant sizing + z-index on :root', () => {
		const root = blockBody(':root');
		const missing = ['--toolbar-height', '--statusbar-height', '--z-dropdown', '--z-modal', '--z-context'].filter(
			(t) => !root.includes(`${t}:`)
		);
		expect(missing, ':root missing invariant tokens').toEqual([]);
	});

	for (const selector of BLOCKS) {
		it(`${selector} defines the full contract`, () => {
			const body = blockBody(selector);
			expect(body, `${selector} block not found`).not.toBe('');
			const missing = REQUIRED.filter((t) => !body.includes(`${t}:`));
			expect(missing, `${selector} missing tokens`).toEqual([]);
		});
	}
});
