/**
 * The outcome of a release step that can fail, in its own module so the steps that run under
 * Node and the ones Deno runs share it without either reaching the other's subprocess code.
 *
 * @module
 */

/** The outcome of a release step that can fail: its value's fields on success, a message
 * naming the failure otherwise. */
export type Result<TValue extends object = object> =
	({ ok: true } & TValue) | { ok: false; error: string };
