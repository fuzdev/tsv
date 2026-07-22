import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	function g() {
		const o = { d: 1 };
		return o.d + d();
	}
	$$renderer.push(`<button>${$.escape(a)}</button>`);
}
