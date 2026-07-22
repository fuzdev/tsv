import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function identity(value) {
		return value;
	}
	let a = identity(1);
	let f = identity;
	$$renderer.push(`<p>${$.escape(a)}${$.escape(f)}</p>`);
}
