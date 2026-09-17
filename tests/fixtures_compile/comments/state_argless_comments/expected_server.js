import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// unset
	let a = void 0; /* nothing yet */
	let b = void 0; // trailing
	function set() {
		// inner note
		a = 1;
		b = 2;
	}
	$$renderer.push(`<button>${$.escape(a)}${$.escape(b)}</button>`);
}
