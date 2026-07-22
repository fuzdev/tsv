import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 0;
	function inc() {
		a += 1;
	}
	$$renderer.push(`<p>${$.escape(a)}</p>`);
}
