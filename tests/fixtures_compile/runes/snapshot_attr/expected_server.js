import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let obj = { a: 1 };
	$$renderer.push(`<pre${$.attr('data-x', $.snapshot(obj))}></pre>`);
}
