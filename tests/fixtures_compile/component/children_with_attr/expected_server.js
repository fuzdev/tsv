import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	Foo($$renderer, {
		a: 'x',
		children: ($$renderer) => {
			$$renderer.push(`<p>hi</p>`);
		},
		$$slots: { default: true }
	});
}
