import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// items
	let items = ['a', 'b'];
	$$renderer.push(`<ul><!--[-->`);
	const each_array = $.ensure_array_like(items);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let item = each_array[$$index];
		$$renderer.push(`<li>${$.escape(item)}</li>`);
	}
	$$renderer.push(`<!--]--></ul>`);
}
