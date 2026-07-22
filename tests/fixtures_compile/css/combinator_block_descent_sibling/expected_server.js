import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	if (x) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<a class="svelte-tsvhash">1</a>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]--><b class="svelte-tsvhash">2</b>`);
}
