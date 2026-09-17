import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = /** @type {number | null} */ (null);
	let b = /* leading */ 1; /* trailing */
	let c = /* callee gap */ 2;
	let d = 3;
	// why three
	function set() {
		a = 1;
		b = 2;
		c = 3;
		d = 4;
	}
	$$renderer.push(`<button>${$.escape(a)}${$.escape(b)}${$.escape(c)}${$.escape(d)}</button>`);
}
