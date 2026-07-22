import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element(
		$$renderer,
		tag,
		() => {
			$$renderer.push(` class="svelte-tsvhash"`);
		},
		() => {
			$$renderer.push(`<span class="svelte-tsvhash"></span>`);
		}
	);
}
