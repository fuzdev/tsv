import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let el;
	$.element($$renderer, tag, void 0, () => {
		$$renderer.push(`hi`);
	});
}
