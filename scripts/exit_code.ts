/**
 * The exit code for a child a signal ended, as a shell reports it: `128 + N` for signal `N`.
 * `node:os` only, so `build:stamped`'s closure can import it (`scripts/roots/cargo_build.ts`).
 *
 * @module
 */

import { constants } from 'node:os';

/** `128 +` the number of the signal named `signal`, `1` for a name this platform doesn't number. */
export function signal_exit_code(signal: string): number {
	const number = (constants.signals as Record<string, number | undefined>)[signal];
	return number === undefined ? 1 : 128 + number;
}
