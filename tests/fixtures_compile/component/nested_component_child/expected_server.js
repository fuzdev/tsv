import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	Foo($$renderer, {
		children: ($$renderer) => {
			Bar($$renderer, {});
		},
		$$slots: { default: true }
	});
}
