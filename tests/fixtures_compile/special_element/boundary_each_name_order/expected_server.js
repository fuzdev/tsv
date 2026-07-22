import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	function failed($$renderer, e) {
		$$renderer.push(`<!--[-->`);
		const each_array_2 = $.ensure_array_like([3]);
		for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
			let c = each_array_2[$$index_2];
			$$renderer.push(`<u>${$.escape(c)}</u>`);
		}
		$$renderer.push(`<!--]-->`);
	}
	$$renderer.boundary({ failed }, ($$renderer) => {
		$$renderer.push(`<!--[!-->`);
		{
			$$renderer.push(`<!--[-->`);
			const each_array_1 = $.ensure_array_like([2]);
			for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
				let b = each_array_1[$$index_1];
				$$renderer.push(`<i>${$.escape(b)}</i>`);
			}
			$$renderer.push(`<!--]-->`);
		}
		$$renderer.push(`<!--]-->`);
	});
	$$renderer.push(`<!--[-->`);
	const each_array_3 = $.ensure_array_like([4]);
	for (let $$index_3 = 0, $$length = each_array_3.length; $$index_3 < $$length; $$index_3++) {
		let d = each_array_3[$$index_3];
		$$renderer.push(`<s>${$.escape(d)}</s>`);
	}
	$$renderer.push(`<!--]-->`);
}
