import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let c = true;
	$$renderer.push(`<p${$.attr('id', c ? 'a' : 'b')} class="svelte-tsvhash">x</p>`);
}
