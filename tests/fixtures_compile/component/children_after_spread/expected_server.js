import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { r } = $$props;
	Foo(
		$$renderer,
		$.spread_props([
			r,
			{
				children: ($$renderer) => {
					$$renderer.push(`<p>hi</p>`);
				},
				$$slots: { default: true }
			}
		])
	);
}
