import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { show } = $$props;
	if (show) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>yes</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
		$$renderer.push(`<p>no</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
