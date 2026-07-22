import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<div>a `);
	$.element($$renderer, tag, void 0, () => {
		$$renderer.push(`x`);
	});
	$$renderer.push(` b</div>`);
}
