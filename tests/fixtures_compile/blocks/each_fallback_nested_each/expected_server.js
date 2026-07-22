import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	const each_array = $.ensure_array_like([1]);
	if (each_array.length !== 0) {
		$$renderer.push('<!--[-->');
		for (let $$index_1 = 0, $$length = each_array.length; $$index_1 < $$length; $$index_1++) {
			let a = each_array[$$index_1];
			$$renderer.push(`<p>${$.escape(a)}</p>`);
		}
	} else {
		$$renderer.push('<!--[!-->');
		$$renderer.push(`<!--[-->`);
		const each_array_1 = $.ensure_array_like([2]);
		for (let $$index = 0, $$length = each_array_1.length; $$index < $$length; $$index++) {
			let b = each_array_1[$$index];
			$$renderer.push(`<i>${$.escape(b)}</i>`);
		}
		$$renderer.push(`<!--]-->`);
	}
	$$renderer.push(`<!--]-->`);
}
