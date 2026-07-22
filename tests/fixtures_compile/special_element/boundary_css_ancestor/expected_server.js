import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<div class="svelte-tsvhash">`);
	$$renderer.push(`<!--[-->`);
	{
		$$renderer.push(`<p class="svelte-tsvhash">x</p>`);
	}
	$$renderer.push(`<!--]-->`);
	$$renderer.push(`</div>`);
}
