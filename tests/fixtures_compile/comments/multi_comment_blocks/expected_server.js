import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	// first
	let a = 1;
	// second
	let b = 2;
	if (a) {
		$$renderer.push('<!--[0-->');
		$$renderer.push(`<p>1</p>`);
	} else {
		$$renderer.push('<!--[-1-->');
	}
	$$renderer.push(`<!--]--> <!--[-->`);
	const each_array = $.ensure_array_like([1]);
	for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
		let n = each_array[$$index];
		$$renderer.push(`<span>2</span>`);
	}
	$$renderer.push(`<!--]-->`);
}
