import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let n = true;
	function s($$renderer) {
		$.await(
			$$renderer,
			p,
			() => {
				$$renderer.push(`load`);
			},
			() => {}
		);
		$$renderer.push(`<!--]-->`);
	}
	s($$renderer);
}
