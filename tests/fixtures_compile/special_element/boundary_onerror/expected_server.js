import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let n = 0;
	$$renderer.push(`<!--[-->`);
	{
		$$renderer.push(`<p>hi</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
