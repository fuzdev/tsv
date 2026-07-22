import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<div class="svelte-tsvhash"></div> `);
	$.element($$renderer, tag, () => {
		$$renderer.push(` class="svelte-tsvhash"`);
	});
}
