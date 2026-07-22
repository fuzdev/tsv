import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let c = true;
	$$renderer.push(`<p data-x="pre-x" class="svelte-tsvhash">x</p>`);
}
