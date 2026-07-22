import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let d = $.derived(() => a * 2);
	function inc() {
		a += 1;
	}
	$$renderer.push(`<p>${$.escape(d())}</p>`);
}
