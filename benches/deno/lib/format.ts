/**
 * Formatting utilities for benchmark output
 */

/** Time unit for consistent display across the session */
export type TimeUnit = 'ms' | 'µs' | 'ns';

/** Determine appropriate time unit based on min value (in ms) */
export function chooseTimeUnit(minMs: number): TimeUnit {
	// If min < 1 of current unit, use next smaller unit
	if (minMs >= 1) return 'ms';
	if (minMs >= 0.001) return 'µs'; // >= 1µs
	return 'ns';
}

/** Format time with specified unit */
export function formatTimeWithUnit(ms: number, unit: TimeUnit): string {
	if (unit === 'ms') {
		if (ms >= 100) {
			return `${Math.round(ms).toLocaleString()}ms`;
		} else if (ms >= 10) {
			return `${ms.toFixed(1)}ms`;
		} else {
			return `${ms.toFixed(2)}ms`;
		}
	} else if (unit === 'µs') {
		const us = ms * 1000;
		if (us >= 100) {
			return `${Math.round(us).toLocaleString()}µs`;
		} else if (us >= 10) {
			return `${us.toFixed(1)}µs`;
		} else {
			return `${us.toFixed(2)}µs`;
		}
	} else {
		const ns = ms * 1_000_000;
		if (ns >= 100) {
			return `${Math.round(ns).toLocaleString()}ns`;
		} else if (ns >= 10) {
			return `${ns.toFixed(1)}ns`;
		} else {
			return `${ns.toFixed(2)}ns`;
		}
	}
}

/** Create a visual bar for comparison */
export function createBar(value: number, max: number, width = 40): string {
	const filled = Math.round((value / max) * width);
	return '█'.repeat(filled) + '░'.repeat(width - filled);
}
