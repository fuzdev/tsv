import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	$$renderer.push(`<!--[!-->`);
	{
		$$renderer.push(`<i>w</i>`);
	}
	$$renderer.push(`<!--]-->`);
	$$renderer.push(`<!--[-->`);
	const each_array_1 = $.ensure_array_like([2]);
	for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
		let z = each_array_1[$$index_1];
		$$renderer.push(`<u>${$.escape(z)}</u>`);
	}
	$$renderer.push(`<!--]-->`);
}
