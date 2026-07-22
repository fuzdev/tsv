import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<b>a</b>`);
	$$renderer.push(`<!--[-->`);
	{
		$$renderer.push(`<p>hi</p>`);
	}
	$$renderer.push(`<!--]-->`);
	$$renderer.push(`<i>z</i>`);
}
