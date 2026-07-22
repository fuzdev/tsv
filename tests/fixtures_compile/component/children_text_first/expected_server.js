import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	Foo($$renderer, {
		children: ($$renderer) => {
			$$renderer.push(`<!---->hi <b>x</b>`);
		},
		$$slots: { default: true }
	});
}
