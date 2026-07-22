import * as $ from 'svelte/internal/server';
export default function Input($$renderer) {
	let n = 1;
	let d = $.derived(() => n * 2);
	function inc() {
		n++;
	}
	Foo($$renderer, { a: d() });
}
