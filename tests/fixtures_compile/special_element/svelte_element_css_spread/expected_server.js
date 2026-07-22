import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element($$renderer, tag, () => {
		$$renderer.push(`${$.attributes({ ...props }, 'svelte-tsvhash')}`);
	});
}
