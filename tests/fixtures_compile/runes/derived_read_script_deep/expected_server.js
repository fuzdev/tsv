import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { a } = $$props;
	let d = $.derived(() => a * 2);
	function f() {
		if (a) {
			for (let i = 0; i < 3; i++) {
				console.log(d() + i);
			}
		}
	}
	$$renderer.push(`<button>${$.escape(a)}</button>`);
}
