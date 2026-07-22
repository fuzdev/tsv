import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let c = true;
	$$renderer.push(`<p${$.attr('data-x', c || 'a')} class="svelte-tsvhash">x</p>`);
}
