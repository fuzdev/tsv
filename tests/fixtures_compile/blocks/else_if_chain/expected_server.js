import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { n } = $$props;
	if (n === 1) {
		$$renderer.push(`<!--[0--><p>one</p>`);
	} else if (n === 2) {
		$$renderer.push(`<!--[1--><p>two</p>`);
	} else {
		$$renderer.push(`<!--[-1--><p>other</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
