import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let a = 1;
	let b = a;
	if (b) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>yes</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
		$$renderer.push(`<p>no</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
