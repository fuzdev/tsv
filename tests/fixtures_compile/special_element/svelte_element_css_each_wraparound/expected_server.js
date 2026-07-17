import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<!--[-->`);
	const each_array = $.ensure_array_like([1]);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let _ = each_array[$$index];
		$.element($$renderer, t, () => {
			$$renderer.push(` class="z svelte-tsvhash"`);
		});
	}
	$$renderer.push(`<!--]-->`);
}
