import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a, $$slots, $$events, ...rest } = $$props;
	$$renderer.push(`<p>${$.escape(a)}</p>`);
}
