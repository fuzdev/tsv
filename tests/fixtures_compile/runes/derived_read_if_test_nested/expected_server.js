import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	if (!d()) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`x`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
}
