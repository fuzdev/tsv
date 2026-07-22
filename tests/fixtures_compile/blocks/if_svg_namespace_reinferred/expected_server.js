import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let x = false;
	$$renderer.push(`<div>`);
	if (x) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<circle></circle><rect></rect>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]--></div>`);
}
