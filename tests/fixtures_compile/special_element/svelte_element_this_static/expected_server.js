import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element($$renderer, 'div', void 0, () => {
		$$renderer.push(`hi`);
	});
}
