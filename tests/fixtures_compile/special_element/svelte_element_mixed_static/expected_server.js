import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$.element($$renderer, 'pre-', void 0, () => {
		$$renderer.push(`x`);
	});
}
