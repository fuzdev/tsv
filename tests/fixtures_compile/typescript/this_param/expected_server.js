import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function f(x) {
		return x;
	}
	let a = f(1);
	$$renderer.push(`<p>${$.escape(a)}</p>`);
}
