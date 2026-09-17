import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let n = 1;
	let double = $.derived(/* compute */ () => n * 2);
	let total = $.derived(() => {
		// sum it
		return n + double();
	});
	function inc() {
		n += 1;
	}
	$$renderer.push(`<button>${$.escape(double())}${$.escape(total())}</button>`);
}
