import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { obj } = $$props;
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
