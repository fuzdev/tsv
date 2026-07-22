import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { obj } = $$props;
		$.element($$renderer, obj.tag, void 0, () => {
			$$renderer.push(`hi`);
		});
	});
}
