import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { n } = $$props;
	if (n === 1) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>one</p>`);
	} else if (n === 2) {
		$$renderer.push('<!--[1-->');
		$$renderer.push(`<p>two</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
		$$renderer.push(`<p>other</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
