import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<p${$.attr('data-x', 0)} class="svelte-tsvhash">x</p>`);
}
