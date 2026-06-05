import { describe, it, expect } from 'vitest';
import { formatDuration, formatTotalDuration, formatSize } from './format';

describe('formatDuration', () => {
	it('formats duration in mm:ss format', () => {
		expect(formatDuration(null)).toBe('');
		expect(formatDuration(undefined)).toBe('');
		expect(formatDuration(0)).toBe('0:00');
		expect(formatDuration(45)).toBe('0:45');
		expect(formatDuration(65)).toBe('1:05');
		expect(formatDuration(3665)).toBe('61:05');
	});
});

describe('formatTotalDuration', () => {
	it('formats positive duration under 60 seconds in seconds', () => {
		expect(formatTotalDuration(0.5)).toBe('1s');
		expect(formatTotalDuration(45)).toBe('45s');
		expect(formatTotalDuration(59.4)).toBe('59s');
	});

	it('formats duration over 60 seconds in minutes/hours', () => {
		expect(formatTotalDuration(0)).toBe('0m');
		expect(formatTotalDuration(60)).toBe('1m');
		expect(formatTotalDuration(65)).toBe('1m');
		expect(formatTotalDuration(3599)).toBe('59m');
		expect(formatTotalDuration(3600)).toBe('1h 0m');
		expect(formatTotalDuration(3665)).toBe('1h 1m');
	});
});

describe('formatSize', () => {
	it('formats bytes into readable size strings', () => {
		expect(formatSize(500)).toBe('500 B');
		expect(formatSize(1024)).toBe('1 KB');
		expect(formatSize(1024 * 1024)).toBe('1.0 MB');
		expect(formatSize(1024 * 1024 * 1024)).toBe('1.00 GB');
	});
});
