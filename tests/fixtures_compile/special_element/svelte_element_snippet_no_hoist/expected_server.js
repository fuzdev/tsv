import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { name } = $$props;
	function s($$renderer) {
		$.element($$renderer, name, void 0, () => {
			$$renderer.push(`x`);
		});
	}
	s($$renderer);
}
