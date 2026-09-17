import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { show } = $$props;
	if (show) {
		$$renderer.push(`<!--[0--><p>yes</p>`);
	} else {
		$$renderer.push(`<!--[-1--><p>no</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
