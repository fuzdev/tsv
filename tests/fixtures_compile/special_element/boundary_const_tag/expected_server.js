import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<!--[-->`);
	{
		const x = 1;
		$$renderer.push(`<p>1</p>`);
	}
	$$renderer.push(`<!--]-->`);
}
