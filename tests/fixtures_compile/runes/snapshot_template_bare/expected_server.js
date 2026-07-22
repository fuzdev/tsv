import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = { a: 1 };
	$$renderer.push(`<!---->${$.escape($.snapshot(obj))}`);
}
