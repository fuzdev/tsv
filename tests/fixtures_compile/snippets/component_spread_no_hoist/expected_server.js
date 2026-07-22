import * as $ from 'svelte/internal/server';
import Foo from './Foo.svelte';
export default function Input($$renderer) {
	let n = { a: 1 };
	function s($$renderer) {
		Foo($$renderer, $.spread_props([n]));
	}
	s($$renderer);
}
