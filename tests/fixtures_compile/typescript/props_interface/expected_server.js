import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a, b } = $$props;
	$$renderer.push(`<p>${$.escape(a)}${$.escape(b)}</p>`);
}
