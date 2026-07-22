import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like(xs);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let x = each_array[$$index];
		$$renderer.push(`<a class="svelte-tsvhash">1</a><b class="svelte-tsvhash">2</b>`);
	}
	$$renderer.push(`<!--]-->`);
}
