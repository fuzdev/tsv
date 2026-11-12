/**
 * The outcome of a release step that can fail, in its own module, importing nothing, so every
 * release module shares it whatever runtime runs it and whatever it can reach.
 *
 * @module
 */

/** The outcome of a release step that can fail: its value's fields on success, a message
 * naming the failure otherwise. */
export type Result<TValue extends object = object> =
	({ ok: true } & TValue) | { ok: false; error: string };
