import * as $ from 'svelte/internal/server';
import Foo from './Foo.svelte';
export default function Input($$renderer) {
	Foo($$renderer, {
		children: ($$renderer) => {
			$.element($$renderer, tag, void 0, () => {
				$$renderer.push(`x`);
			});
		},
		$$slots: { default: true }
	});
}
