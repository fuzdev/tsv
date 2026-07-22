import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		$.await(
			$$renderer,
			p,
			() => {
				$$renderer.push(`x`);
			},
			() => {}
		);
		$$renderer.push(`<!--]-->`);
	});
}
