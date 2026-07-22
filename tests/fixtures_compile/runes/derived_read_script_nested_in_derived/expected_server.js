import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	let d2 = $.derived(() => d() + 1);
	$$renderer.push(`<button>${$.escape(a)}</button>`);
}
