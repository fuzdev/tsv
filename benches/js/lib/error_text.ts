/**
 * Error text helpers shared by the comparison tools. Node-modules-free.
 *
 * @module
 */

/**
 * The first line of a thrown value's message — a rejection's message can carry a
 * multi-line source code frame, which a per-file report line must not print.
 */
export function first_line(e: unknown): string {
	return String(e instanceof Error ? e.message : e).split('\n')[0];
}
