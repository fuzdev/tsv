import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// count
	let n = 3;
	if (n > 0) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>3</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]-->`);
}
