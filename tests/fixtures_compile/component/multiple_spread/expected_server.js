import * as $ from 'svelte/internal/server';
export default function Input($$renderer, $$props) {
	let { r, s } = $$props;
	Foo($$renderer, $.spread_props([r, s]));
}
