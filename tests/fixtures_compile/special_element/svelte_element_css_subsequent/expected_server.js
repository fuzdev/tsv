import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element($$renderer, t, () => {
		$$renderer.push(` class="x svelte-tsvhash"`);
	});
	$$renderer.push(` <p></p> <span class="foo svelte-tsvhash"></span>`);
}
