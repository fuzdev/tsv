import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { h } = $$props;
	$$renderer.push(`<button${$.attr('onclick', h)}>x</button>`);
}
