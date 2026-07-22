import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let c = true;
	$$renderer.push(`<p${$.attr('data-x', c && 'v')} class="svelte-tsvhash">x</p>`);
}
