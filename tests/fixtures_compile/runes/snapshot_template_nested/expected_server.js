import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = { a: 1 };
	function wrap(x) {
		return x;
	}
	$$renderer.push(`<!---->${$.escape(wrap($.snapshot(obj)))}`);
}
