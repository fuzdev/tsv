import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { x } = $$props;
	Foo($$renderer, {
		children: ($$renderer) => {
			if (x) {
				$$renderer.push('<!--[0-->');
				$$renderer.push(`<p>a</p>`);
			} else {
				$$renderer.push('<!--[-1-->');
			}
			$$renderer.push(`<!--]-->`);
		},
		$$slots: { default: true }
	});
}
