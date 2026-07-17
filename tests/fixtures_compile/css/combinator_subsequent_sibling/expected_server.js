import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(
		`<a class="svelte-tsvhash">1</a><b class="svelte-tsvhash">2</b><b class="svelte-tsvhash">3</b>`
	);
}
